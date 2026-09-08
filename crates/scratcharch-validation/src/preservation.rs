//! Transform-preservation engine.
//!
//! A ScratchGraph optimization pass changes the IR — that is its job — so a
//! plain `N(before) == N(after)` check is both too weak and too strong. What
//! must hold is that the pass commits no *disallowed* change. The verdicts
//! here are the framework's own, written independently of the pass code they
//! audit:
//!
//! - [`dce_preserves_liveness`] — dead-script elimination may remove only
//!   scripts/procedures that are provably dead. It is forbidden from removing
//!   a reachable broadcast receiver, a live procedure, or from changing which
//!   receivers a broadcast reaches.
//! - [`constant_folding_reaches_canonical_fold`] — constant folding may
//!   replace a fully-constant numeric arithmetic expression with its IEEE-754
//!   value, and nothing else. It must not alter control structure, a boolean
//!   context (comparisons are never folded), a broadcast, or variable/list
//!   usage.
//! - [`variable_analysis_keeps_referenced`] — variable analysis may drop only
//!   declarations whose name no remaining code reads or writes.
//! - [`empty_block_removal_matches`] — empty-block removal may drop only empty
//!   control wrappers, never a block with a non-empty body.
//!
//! Every verdict compares normalized forms, so block IDs, declaration order,
//! and target order never matter.

use std::collections::{HashMap, HashSet};

use scratcharch_scratchgraph::ir::{EventHat, Expr, Project, Stmt};
use scratcharch_scratchgraph::SemanticNormalizer;

fn canon() -> SemanticNormalizer {
    SemanticNormalizer::new()
}

fn proc_body<'a>(p: &'a Project, target: &str, name: &str) -> Option<&'a Vec<Stmt>> {
    if target == p.stage.name {
        p.stage
            .procedures
            .iter()
            .find(|pr| pr.prototype.name == name)
            .map(|pr| &pr.body)
    } else {
        p.sprites
            .iter()
            .find(|s| s.name == target)
            .and_then(|s| {
                s.procedures
                    .iter()
                    .find(|pr| pr.prototype.name == name)
                    .map(|pr| &pr.body)
            })
    }
}

/// All procedures of one target.
fn procs_of<'a>(p: &'a Project, target: &str) -> &'a [scratcharch_scratchgraph::Procedure] {
    if target == p.stage.name {
        &p.stage.procedures
    } else {
        p.sprites
            .iter()
            .find(|s| s.name == target)
            .map(|s| s.procedures.as_slice())
            .unwrap_or(&[])
    }
}

/// `(target name, hat, body)` for every script in the project.
fn all_scripts(p: &Project) -> Vec<(String, EventHat, Vec<Stmt>)> {
    let mut out = Vec::new();
    for s in &p.stage.scripts {
        out.push((p.stage.name.clone(), s.entry.hat.clone(), s.entry.body.clone()));
    }
    for sp in &p.sprites {
        for s in &sp.scripts {
            out.push((sp.name.clone(), s.entry.hat.clone(), s.entry.body.clone()));
        }
    }
    out
}

/// Literal message names sent anywhere, and whether any send has a non-literal
/// (dynamic) message. Recurse into control bodies so a send inside an If or a
/// loop counts.
fn collect_sends(p: &Project) -> (HashSet<String>, bool) {
    fn sends(stmts: &[Stmt], literals: &mut HashSet<String>, dynamic: &mut bool) {
        for stmt in stmts {
            match stmt {
                Stmt::Broadcast { message } => match message {
                    Expr::Literal(scratcharch_scratchgraph::Value::String(m)) => {
                        literals.insert(m.clone());
                    }
                    _ => *dynamic = true,
                },
                Stmt::If { then_body, else_body, .. } => {
                    sends(then_body, literals, dynamic);
                    sends(else_body, literals, dynamic);
                }
                Stmt::Repeat { body, .. }
                | Stmt::RepeatUntil { body, .. }
                | Stmt::Forever { body } => sends(body, literals, dynamic),
                _ => {}
            }
        }
    }
    let mut literals = HashSet::new();
    let mut dynamic = false;
    for (_, _, body) in all_scripts(p) {
        sends(&body, &mut literals, &mut dynamic);
    }
    for pr in &p.stage.procedures {
        sends(&pr.body, &mut literals, &mut dynamic);
    }
    for sp in &p.sprites {
        for pr in &sp.procedures {
            sends(&pr.body, &mut literals, &mut dynamic);
        }
    }
    (literals, dynamic)
}

fn calls_of(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Call { proc, .. } => {
                out.insert(proc.clone());
            }
            Stmt::If { then_body, else_body, .. } => {
                calls_of(then_body, out);
                calls_of(else_body, out);
            }
            Stmt::Repeat { body, .. }
            | Stmt::RepeatUntil { body, .. }
            | Stmt::Forever { body } => calls_of(body, out),
            _ => {}
        }
    }
}

/// Target names (stage first, then sprites).
fn target_names(p: &Project) -> Vec<&str> {
    let mut out = vec![p.stage.name.as_str()];
    for s in &p.sprites {
        out.push(s.name.as_str());
    }
    out
}

/// Procedures per target that are live, mirroring DCE exactly: a procedure is
/// kept when its name is called from a live script of the owning target, or
/// from any procedure body of that target (so recursion cycles and procedures
/// called only from other procedure bodies stay).
fn reachable_procedures(p: &Project) -> HashMap<String, HashSet<String>> {
    let (sent, dynamic) = collect_sends(p);
    let live_hat = |hat: &EventHat| match hat {
        EventHat::BroadcastReceived(m) => dynamic || sent.contains(m),
        _ => true,
    };
    // Calls from live scripts, per target.
    let mut script_calls: HashMap<String, HashSet<String>> = HashMap::new();
    for (target, hat, body) in all_scripts(p) {
        if live_hat(&hat) {
            let mut calls = HashSet::new();
            calls_of(&body, &mut calls);
            script_calls.entry(target).or_default().extend(calls);
        }
    }
    let mut per_target: HashMap<String, HashSet<String>> = HashMap::new();
    for target in target_names(p) {
        let mut live = script_calls.remove(target).unwrap_or_default();
        for pr in procs_of(p, target) {
            let mut calls = HashSet::new();
            calls_of(&pr.body, &mut calls);
            live.extend(calls);
        }
        per_target.insert(target.to_string(), live);
    }
    per_target
}

/// Fingerprint of one whole script (target + hat + normalized body) for
/// matching an unchanged script across a pass.
fn script_key(target: &str, hat: &EventHat, body: &[Stmt]) -> String {
    let n = canon();
    format!("{target}::{hat:?}::{}", n.fingerprint(&n.normalize_stmts(body)))
}

/// **DCE soundness.** `after` must still contain every script and procedure
/// that was live in `before`: the live broadcast receivers (their message is
/// sent by code that remains, or the broadcast is dynamic), all live scripts
/// under always-on hats, and every reachable procedure per target.
pub fn dce_preserves_liveness(before: &Project, after: &Project) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    let (sent, dynamic) = collect_sends(before);
    let live_hat = |hat: &EventHat| match hat {
        EventHat::BroadcastReceived(m) => dynamic || sent.contains(m),
        _ => true,
    };

    // Live scripts (by target + fingerprint) must still exist.
    let mut want: HashMap<(String, String), usize> = HashMap::new();
    for (target, hat, body) in all_scripts(before) {
        if live_hat(&hat) {
            *want.entry((target.clone(), script_key(&target, &hat, &body))).or_default() += 1;
        }
    }
    let mut have: HashMap<(String, String), usize> = HashMap::new();
    for (target, hat, body) in all_scripts(after) {
        *have.entry((target.clone(), script_key(&target, &hat, &body))).or_default() += 1;
    }
    for ((target, key), count) in want {
        if have.get(&(target.clone(), key.clone())).copied().unwrap_or(0) < count {
            errors.push(format!("live script removed on target `{target}`: {key}"));
        }
    }

    // Live procedures per target must still exist.
    for (target, live) in reachable_procedures(before) {
        for name in live {
            if proc_body(after, &target, &name).is_none() {
                errors.push(format!("live procedure `{name}` removed on target `{target}`"));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// All variable names referenced anywhere in `p` (recursive).
pub fn referenced_variable_names(p: &Project) -> HashSet<String> {
    fn stmt_vars(s: &Stmt, out: &mut HashSet<String>) {
        match s {
            Stmt::SetVariable { var, value } => {
                out.insert(var.clone());
                expr_vars(value, out);
            }
            Stmt::ChangeVariable { var, delta } => {
                out.insert(var.clone());
                expr_vars(delta, out);
            }
            Stmt::AddToList { value, .. } => expr_vars(value, out),
            Stmt::SetListItem { index, value, .. } => {
                expr_vars(index, out);
                expr_vars(value, out);
            }
            Stmt::DeleteListItem { index, .. } => expr_vars(index, out),
            Stmt::InsertListItem { index, value, .. } => {
                expr_vars(index, out);
                expr_vars(value, out);
            }
            Stmt::Broadcast { message } => expr_vars(message, out),
            Stmt::Call { args, .. } => {
                for a in args {
                    expr_vars(a, out);
                }
            }
            Stmt::Expr(e) => expr_vars(e, out),
            Stmt::If { condition, then_body, else_body, .. } => {
                expr_vars(condition, out);
                for b in then_body {
                    stmt_vars(b, out);
                }
                for b in else_body {
                    stmt_vars(b, out);
                }
            }
            Stmt::Repeat { times, body, .. } => {
                expr_vars(times, out);
                for b in body {
                    stmt_vars(b, out);
                }
            }
            Stmt::RepeatUntil { condition, body, .. } => {
                expr_vars(condition, out);
                for b in body {
                    stmt_vars(b, out);
                }
            }
            Stmt::Forever { body } => {
                for b in body {
                    stmt_vars(b, out);
                }
            }
            _ => {}
        }
    }
    fn expr_vars(e: &Expr, out: &mut HashSet<String>) {
        match e {
            Expr::Variable(name) => {
                out.insert(name.clone());
            }
            Expr::ListItem { index, .. } => expr_vars(index, out),
            Expr::Operator { args, .. } => {
                for a in args {
                    expr_vars(a, out);
                }
            }
            Expr::HeapLoad { addr } => expr_vars(addr, out),
            Expr::HeapIndex { base, offset } => {
                expr_vars(base, out);
                expr_vars(offset, out);
            }
            Expr::Literal(_)
            | Expr::List(_)
            | Expr::ListLength { .. }
            | Expr::ProcedureParam(_)
            | Expr::FrameBase
            | Expr::FrameGet { .. } => {}
        }
    }
    let mut out = HashSet::new();
    for (_, _, body) in all_scripts(p) {
        for s in &body {
            stmt_vars(s, &mut out);
        }
    }
    for pr in &p.stage.procedures {
        for s in &pr.body {
            stmt_vars(s, &mut out);
        }
    }
    for sp in &p.sprites {
        for pr in &sp.procedures {
            for s in &pr.body {
                stmt_vars(s, &mut out);
            }
        }
    }
    out
}

fn declared_variable_names(p: &Project) -> HashSet<String> {
    let mut out = HashSet::new();
    for v in &p.stage.variables {
        out.insert(v.name.clone());
    }
    for sp in &p.sprites {
        for v in &sp.variables {
            out.insert(v.name.clone());
        }
    }
    out
}

/// **Variable-analysis soundness.** The pass must not change which names are
/// referenced (it only edits declarations), every referenced name must still be
/// declared after the pass, and only unreferenced names may have been dropped.
pub fn variable_analysis_keeps_referenced(before: &Project, after: &Project) -> Result<(), String> {
    let before_ref = referenced_variable_names(before);
    let after_ref = referenced_variable_names(after);
    if before_ref != after_ref {
        return Err("variable analysis changed the set of referenced variable names".into());
    }
    let after_decl = declared_variable_names(after);
    let mut missing: Vec<&String> = after_ref.iter().filter(|n| !after_decl.contains(*n)).collect();
    missing.sort();
    if !missing.is_empty() {
        return Err(format!(
            "referenced variable(s) no longer declared: {}",
            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(())
}

/// **Constant-folding soundness.** `after` must equal the project obtained by
/// folding every fully-constant numeric arithmetic expression with IEEE-754
/// evaluation, independently of the pass. Comparison/boolean operators are
/// never folded, so this also guarantees boolean contexts are untouched.
pub fn constant_folding_reaches_canonical_fold(before: &Project, after: &Project) -> Result<(), String> {
    let expected = fold_all_constant_arithmetic(before);
    let n = canon();
    if n.normalize(after) == n.normalize(&expected) {
        Ok(())
    } else {
        Err("constant folding changed something other than folding constant numeric expressions".into())
    }
}

/// Independent constant folding: bottom-up `+ - * /` over all-numeric-literal
/// operands using IEEE-754 `f64`; division by zero is never folded.
fn fold_all_constant_arithmetic(p: &Project) -> Project {
    fn try_fold(e: &mut Expr) {
        let (opcode, args) = match e {
            Expr::Operator { opcode, args } => (opcode.clone(), args),
            _ => return,
        };
        let mut nums = Vec::new();
        for a in args.iter() {
            match a {
                Expr::Literal(scratcharch_scratchgraph::Value::Number(n)) => nums.push(*n),
                _ => return,
            }
        }
        if nums.len() < 2 {
            return;
        }
        let value = match opcode.as_str() {
            "operator_add" => Some(nums.iter().sum()),
            "operator_subtract" => Some(nums[0] - nums[1]),
            "operator_multiply" => Some(nums.iter().product()),
            "operator_divide" if nums[1] != 0.0 => Some(nums[0] / nums[1]),
            _ => None,
        };
        if let Some(v) = value {
            *e = Expr::number(v);
        }
    }
    fn fold_expr(e: &mut Expr) {
        match e {
            Expr::Operator { args, .. } => {
                for a in args.iter_mut() {
                    fold_expr(a);
                }
                try_fold(e);
            }
            Expr::ListItem { index, .. } => fold_expr(index),
            Expr::HeapLoad { addr } => fold_expr(addr),
            Expr::HeapIndex { base, offset } => {
                fold_expr(base);
                fold_expr(offset);
            }
            _ => {}
        }
    }
    fn fold_stmt(s: &mut Stmt) {
        match s {
            Stmt::SetVariable { value, .. } => fold_expr(value),
            Stmt::ChangeVariable { delta, .. } => fold_expr(delta),
            Stmt::AddToList { value, .. } => fold_expr(value),
            Stmt::SetListItem { index, value, .. } => {
                fold_expr(index);
                fold_expr(value);
            }
            Stmt::DeleteListItem { index, .. } => fold_expr(index),
            Stmt::InsertListItem { index, value, .. } => {
                fold_expr(index);
                fold_expr(value);
            }
            Stmt::Broadcast { message } => fold_expr(message),
            Stmt::Call { args, .. } => {
                for a in args.iter_mut() {
                    fold_expr(a);
                }
            }
            Stmt::If { condition, then_body, else_body, .. } => {
                fold_expr(condition);
                for s in then_body.iter_mut() {
                    fold_stmt(s);
                }
                for s in else_body.iter_mut() {
                    fold_stmt(s);
                }
            }
            Stmt::Repeat { times, body, .. } => {
                fold_expr(times);
                for s in body.iter_mut() {
                    fold_stmt(s);
                }
            }
            Stmt::RepeatUntil { condition, body, .. } => {
                fold_expr(condition);
                for s in body.iter_mut() {
                    fold_stmt(s);
                }
            }
            Stmt::Forever { body } => {
                for s in body.iter_mut() {
                    fold_stmt(s);
                }
            }
            _ => {}
        }
    }
    fn fold_bodies(p: &mut Project) {
        for s in &mut p.stage.scripts {
            for st in &mut s.entry.body {
                fold_stmt(st);
            }
        }
        for pr in &mut p.stage.procedures {
            for st in &mut pr.body {
                fold_stmt(st);
            }
        }
        for sp in &mut p.sprites {
            for s in &mut sp.scripts {
                for st in &mut s.entry.body {
                    fold_stmt(st);
                }
            }
            for pr in &mut sp.procedures {
                for st in &mut pr.body {
                    fold_stmt(st);
                }
            }
        }
    }
    let mut out = p.clone();
    fold_bodies(&mut out);
    out
}

/// **Empty-block-removal soundness.** `after` must equal `before` with exactly
/// the empty control wrappers removed (independent re-implementation); a block
/// with any non-empty body must survive.
pub fn empty_block_removal_matches(before: &Project, after: &Project) -> Result<(), String> {
    let expected = remove_empty_wrappers(before);
    let n = canon();
    if n.normalize(after) == n.normalize(&expected) {
        Ok(())
    } else {
        Err("empty-block removal changed something other than empty control wrappers".into())
    }
}

fn remove_empty_wrappers(p: &Project) -> Project {
    fn strip(body: &mut Vec<Stmt>) {
        let mut i = 0;
        while i < body.len() {
            match &mut body[i] {
                Stmt::If { then_body, else_body, .. } => {
                    strip(then_body);
                    strip(else_body);
                    if then_body.is_empty() && else_body.is_empty() {
                        body.remove(i);
                        continue;
                    }
                }
                Stmt::Repeat { body: b, .. } => {
                    strip(b);
                    if b.is_empty() {
                        body.remove(i);
                        continue;
                    }
                }
                Stmt::RepeatUntil { body: b, .. } => {
                    strip(b);
                    if b.is_empty() {
                        body.remove(i);
                        continue;
                    }
                }
                Stmt::Forever { body: b } => {
                    strip(b);
                    if b.is_empty() {
                        body.remove(i);
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    let mut out = p.clone();
    for s in &mut out.stage.scripts {
        strip(&mut s.entry.body);
    }
    for pr in &mut out.stage.procedures {
        strip(&mut pr.body);
    }
    for sp in &mut out.sprites {
        for s in &mut sp.scripts {
            strip(&mut s.entry.body);
        }
        for pr in &mut sp.procedures {
            strip(&mut pr.body);
        }
    }
    out
}
