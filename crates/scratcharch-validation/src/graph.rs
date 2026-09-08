//! Lightweight graph validation for ScratchGraph projects.
//!
//! A project that parses is not automatically *well-formed*: ScratchGraph
//! scripts reference variables, lists, and procedures by name, and nothing in
//! the format itself forces those names to resolve. This module checks the
//! reference integrity a semantics-preserving toolchain depends on:
//!
//! - target names are unique (sprites and stage);
//! - every variable a script/procedure reads or writes is declared on the
//!   referencing target or on the stage (the global pool);
//! - every list access targets a declared list, likewise target-or-stage;
//! - every `Call` names a procedure defined on the referencing target or on
//!   the stage.
//!
//! Broadcast *messages* are deliberately not checked: Scratch lets a project
//! broadcast a message no target declares (SCRATCH_SEMANTICS.md §2.2 treats
//! them as a project-global name set), so a receive hat with no matching
//! declaration is still well-formed.

use std::collections::{HashMap, HashSet};

use scratcharch_scratchgraph::ir::{Expr, Project, Stmt};

#[derive(Default)]
struct Uses {
    vars: HashSet<String>,
    lists: HashSet<String>,
    procs: HashSet<String>,
}

fn collect_uses(body: &[Stmt], u: &mut Uses) {
    fn walk(body: &[Stmt], u: &mut Uses) {
        for s in body {
            match s {
                Stmt::SetVariable { var, value } => {
                    u.vars.insert(var.clone());
                    expr(value, u);
                }
                Stmt::ChangeVariable { var, delta } => {
                    u.vars.insert(var.clone());
                    expr(delta, u);
                }
                Stmt::AddToList { list, value } => {
                    u.lists.insert(list.clone());
                    expr(value, u);
                }
                Stmt::DeleteAllOfList { list } => {
                    u.lists.insert(list.clone());
                }
                Stmt::SetListItem { list, index, value } => {
                    u.lists.insert(list.clone());
                    expr(index, u);
                    expr(value, u);
                }
                Stmt::DeleteListItem { list, index } => {
                    u.lists.insert(list.clone());
                    expr(index, u);
                }
                Stmt::InsertListItem { list, index, value } => {
                    u.lists.insert(list.clone());
                    expr(index, u);
                    expr(value, u);
                }
                Stmt::Call { proc, args } => {
                    u.procs.insert(proc.clone());
                    for a in args {
                        expr(a, u);
                    }
                }
                Stmt::Expr(e) => expr(e, u),
                Stmt::If { condition, then_body, else_body, .. } => {
                    expr(condition, u);
                    walk(then_body, u);
                    walk(else_body, u);
                }
                Stmt::Repeat { times, body } => {
                    expr(times, u);
                    walk(body, u);
                }
                Stmt::RepeatUntil { condition, body } => {
                    expr(condition, u);
                    walk(body, u);
                }
                Stmt::Forever { body } => walk(body, u),
                Stmt::Broadcast { message } => expr(message, u),
                Stmt::EnterFrame { .. } | Stmt::PopFrame { .. } | Stmt::Stop { .. } => {}
                Stmt::FrameSet { value, .. } => expr(value, u),
                Stmt::HeapAlloc { size, .. } => expr(size, u),
            }
        }
    }
    fn expr(e: &Expr, u: &mut Uses) {
        match e {
            Expr::Variable(name) => {
                u.vars.insert(name.clone());
            }
            Expr::List(list) => {
                u.lists.insert(list.clone());
            }
            Expr::ListItem { list, index } => {
                u.lists.insert(list.clone());
                expr(index, u);
            }
            Expr::ListLength { list } => {
                u.lists.insert(list.clone());
            }
            Expr::Operator { args, .. } => {
                for a in args {
                    expr(a, u);
                }
            }
            Expr::HeapLoad { addr } => expr(addr, u),
            Expr::HeapIndex { base, offset } => {
                expr(base, u);
                expr(offset, u);
            }
            Expr::Literal(_)
            | Expr::ProcedureParam(_)
            | Expr::FrameBase
            | Expr::FrameGet { .. } => {}
        }
    }
    walk(body, u);
}

/// Body uses of one target.
fn body_uses(body: &[Stmt]) -> Uses {
    let mut u = Uses::default();
    collect_uses(body, &mut u);
    u
}

fn proc_names(project: &Project, target: &str) -> HashSet<String> {
    let procs = if target == project.stage.name {
        &project.stage.procedures
    } else {
        match project.sprites.iter().find(|s| s.name == target) {
            Some(s) => &s.procedures,
            None => return HashSet::new(),
        }
    };
    procs.iter().map(|p| p.prototype.name.clone()).collect()
}

fn declared_names<'a>(names: impl Iterator<Item = &'a str>) -> HashSet<String> {
    names.map(String::from).collect()
}

/// Validate reference integrity. Returns `Err(issues)` listing each violation.
pub fn validate(project: &Project) -> Result<(), Vec<String>> {
    let mut issues: Vec<String> = Vec::new();

    // Target names must be unique.
    let mut seen: HashSet<&str> = HashSet::new();
    for name in std::iter::once(project.stage.name.as_str())
        .chain(project.sprites.iter().map(|s| s.name.as_str()))
    {
        if !seen.insert(name) {
            issues.push(format!("duplicate target name `{name}`"));
        }
    }

    let stage_vars = declared_names(project.stage.variables.iter().map(|v| v.name.as_str()));
    let stage_lists = declared_names(project.stage.lists.iter().map(|l| l.name.as_str()));

    // Per-target uses.
    let mut per_target: HashMap<String, Uses> = HashMap::new();
    let mut collect_target = |target: &str, scripts: &[Vec<Stmt>]| {
        let mut u = Uses::default();
        for body in scripts {
            let body_uses = body_uses(body);
            u.vars.extend(body_uses.vars);
            u.lists.extend(body_uses.lists);
            u.procs.extend(body_uses.procs);
        }
        per_target.insert(target.to_string(), u);
    };
    let mut stage_bodies: Vec<Vec<Stmt>> = project
        .stage
        .scripts
        .iter()
        .map(|s| s.entry.body.clone())
        .collect();
    stage_bodies.extend(project.stage.procedures.iter().map(|p| p.body.clone()));
    collect_target(&project.stage.name, &stage_bodies);
    for s in &project.sprites {
        let mut bodies: Vec<Vec<Stmt>> = s.scripts.iter().map(|sc| sc.entry.body.clone()).collect();
        bodies.extend(s.procedures.iter().map(|p| p.body.clone()));
        collect_target(&s.name, &bodies);
    }

    let own_vars = |target: &str| -> HashSet<String> {
        let mut out = stage_vars.clone();
        if target != project.stage.name {
            if let Some(s) = project.sprites.iter().find(|s| s.name == target) {
                out.extend(s.variables.iter().map(|v| v.name.clone()));
            }
        }
        out
    };
    let own_lists = |target: &str| -> HashSet<String> {
        let mut out = stage_lists.clone();
        if target != project.stage.name {
            if let Some(s) = project.sprites.iter().find(|s| s.name == target) {
                out.extend(s.lists.iter().map(|l| l.name.clone()));
            }
        }
        out
    };

    let mut targets: Vec<&String> = per_target.keys().collect();
    targets.sort();
    for target in targets {
        let u = &per_target[target];
        let declared_vars = own_vars(target);
        let declared_lists = own_lists(target);
        let declared_procs = proc_names(project, target);

        let mut vars: Vec<&String> = u.vars.iter().collect();
        vars.sort();
        for name in vars {
            if !declared_vars.contains(name) {
                issues.push(format!(
                    "variable `{name}` referenced on target `{target}` is not declared there or on the stage"
                ));
            }
        }
        let mut lists: Vec<&String> = u.lists.iter().collect();
        lists.sort();
        for name in lists {
            if !declared_lists.contains(name) {
                issues.push(format!(
                    "list `{name}` referenced on target `{target}` is not declared there or on the stage"
                ));
            }
        }
        let mut procs: Vec<&String> = u.procs.iter().collect();
        procs.sort();
        for name in procs {
            if !declared_procs.contains(name) {
                issues.push(format!(
                    "procedure `{name}` called on target `{target}` is not defined there or on the stage"
                ));
            }
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}
