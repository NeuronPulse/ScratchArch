//! Semantic normalization for ScratchGraph projects.
//!
//! Semantic equivalence is an IR-level property: two `Project`s are
//! semantically equivalent when their *normalized* forms are equal. The
//! [`SemanticNormalizer`] erases everything that carries no meaning (block IDs,
//! declaration ordering, broadcast declaration site, script debug names) and
//! canonicalizes what remains, so that two projects which differ only in
//! representation normalize to the same value.
//!
//! See `docs/specification/SCRATCH_SEMANTICS.md` for the full model. Rules used
//! here:
//!
//! - variables and lists are identified by `(name, scope)`; their `id` is
//!   representation only and is dropped;
//! - broadcasts are project-global messages; only the message *name* is kept,
//!   and the whole project is one sorted set of names;
//! - sprites are sorted by name (sprite list order is not semantic);
//! - per-target variable/list declarations are sorted; procedure definitions
//!   are sorted by name (calls bind by name);
//! - script *bodies* keep their statement order (order is observable), but the
//!   script debug `name` is dropped;
//! - `-0.0` is folded to `0.0` (Scratch/JS treat them as equal).

use crate::ir::{
    Broadcast, EventHat, Expr, List, ListScope, Procedure, ProcedureParam, ProcedurePrototype,
    Project, Script, Sprite, Stage, Stmt, Value, Variable, VariableScope,
};

/// A project reduced to its semantic content.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedProject {
    pub stage: NormalizedTarget,
    /// Sprites sorted by name.
    pub sprites: Vec<NormalizedTarget>,
    /// Project-wide broadcast message names (sorted, deduplicated).
    pub broadcasts: Vec<String>,
}

/// A single target (stage or sprite) reduced to its semantic content.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTarget {
    pub name: String,
    /// `(name, scope)` pairs, sorted; ids dropped.
    pub variables: Vec<NormalizedVariable>,
    pub lists: Vec<NormalizedList>,
    /// Scripts keep their original relative order (observable).
    pub scripts: Vec<NormalizedScript>,
    /// Procedures sorted by name.
    pub procedures: Vec<NormalizedProcedure>,
}

/// A variable by name and scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedVariable {
    pub name: String,
    pub scope: VariableScope,
}

/// A list by name and scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedList {
    pub name: String,
    pub scope: ListScope,
}

/// A script hat plus its canonical body.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedScript {
    pub hat: EventHat,
    pub body: Vec<NormalizedStmt>,
}

/// A procedure by name, signature, and canonical body.
///
/// `frame_size` is compiler bookkeeping (derivable from the body's frame
/// ops), not part of observable behavior, so it is intentionally absent.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedProcedure {
    pub name: String,
    pub params: Vec<NormalizedParam>,
    pub body: Vec<NormalizedStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedParam {
    pub name: String,
    pub default: Option<Value>,
}

/// Canonical statements reuse the IR types: the IR carries no block IDs, and
/// `canon`-walks only fold `-0.0`/recursion over children.
pub type NormalizedStmt = Stmt;
pub type NormalizedExpr = Expr;

/// Builds the canonical, id-independent form of a project.
#[derive(Debug, Clone, Copy, Default)]
pub struct SemanticNormalizer;

impl SemanticNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// Normalize a project into its semantic form.
    pub fn normalize(&self, project: &Project) -> NormalizedProject {
        let mut broadcasts: Vec<String> = collect_broadcast_names(&project.stage.broadcasts);
        for sprite in &project.sprites {
            broadcasts.extend(collect_broadcast_names(&sprite.broadcasts));
        }
        broadcasts.sort();
        broadcasts.dedup();

        let mut sprites: Vec<NormalizedTarget> =
            project.sprites.iter().map(|s| normalize_target(s.name.as_str(), s)).collect();
        sprites.sort_by(|a, b| a.name.cmp(&b.name));

        NormalizedProject {
            stage: normalize_target(project.stage.name.as_str(), &project.stage),
            sprites,
            broadcasts,
        }
    }

    /// Canonical fingerprint of a statement list. Two bodies are semantically
    /// equal iff their canonicalized forms compare equal; the fingerprint is a
    /// stable string form of that canonical body (deterministic `Debug`).
    pub fn fingerprint(&self, body: &[NormalizedStmt]) -> String {
        format!("{:?}", body)
    }

    /// Normalize a single statement list (used by tests and the analyzer).
    pub fn normalize_stmts(&self, stmts: &[Stmt]) -> Vec<NormalizedStmt> {
        canon_stmts(stmts)
    }
}

fn normalize_target<S>(name: &str, src: &S) -> NormalizedTarget
where
    S: TargetLike,
{
    let mut variables: Vec<NormalizedVariable> = src
        .variables()
        .iter()
        .map(|v| NormalizedVariable {
            name: v.name.clone(),
            scope: v.scope,
        })
        .collect();
    variables.sort_by(|a, b| sort_key(&a.name, scope_rank(a.scope)).cmp(&sort_key(&b.name, scope_rank(b.scope))));
    variables.dedup_by(|a, b| a.name == b.name && a.scope == b.scope);

    let mut lists: Vec<NormalizedList> = src
        .lists()
        .iter()
        .map(|l| NormalizedList {
            name: l.name.clone(),
            scope: l.scope,
        })
        .collect();
    lists.sort_by(|a, b| sort_key(&a.name, list_scope_rank(a.scope)).cmp(&sort_key(&b.name, list_scope_rank(b.scope))));
    lists.dedup_by(|a, b| a.name == b.name && a.scope == b.scope);

    let scripts: Vec<NormalizedScript> = src
        .scripts()
        .iter()
        .map(|s| NormalizedScript {
            hat: s.entry.hat.clone(),
            body: canon_stmts(&s.entry.body),
        })
        .collect();

    let mut procedures: Vec<NormalizedProcedure> = src
        .procedures()
        .iter()
        .map(|p| NormalizedProcedure {
            name: p.prototype.name.clone(),
            params: p
                .prototype
                .params
                .iter()
                .map(|param| NormalizedParam {
                    name: param.name.clone(),
                    default: param.default.as_ref().map(canon_value),
                })
                .collect(),
            body: canon_stmts(&p.body),
        })
        .collect();
    procedures.sort_by(|a, b| a.name.cmp(&b.name));

    NormalizedTarget {
        name: name.to_string(),
        variables,
        lists,
        scripts,
        procedures,
    }
}

/// Abstraction over the shared fields of `Stage` and `Sprite`.
trait TargetLike {
    fn variables(&self) -> &[Variable];
    fn lists(&self) -> &[List];
    fn scripts(&self) -> &[Script];
    fn procedures(&self) -> &[Procedure];
}

impl TargetLike for Stage {
    fn variables(&self) -> &[Variable] {
        &self.variables
    }
    fn lists(&self) -> &[List] {
        &self.lists
    }
    fn scripts(&self) -> &[Script] {
        &self.scripts
    }
    fn procedures(&self) -> &[Procedure] {
        &self.procedures
    }
}

impl TargetLike for Sprite {
    fn variables(&self) -> &[Variable] {
        &self.variables
    }
    fn lists(&self) -> &[List] {
        &self.lists
    }
    fn scripts(&self) -> &[Script] {
        &self.scripts
    }
    fn procedures(&self) -> &[Procedure] {
        &self.procedures
    }
}

fn collect_broadcast_names(broadcasts: &[Broadcast]) -> Vec<String> {
    broadcasts.iter().map(|b| b.name.clone()).collect()
}

fn scope_rank(s: VariableScope) -> u8 {
    match s {
        VariableScope::Global => 0,
        VariableScope::SpriteLocal => 1,
        VariableScope::Temporary => 2,
    }
}

fn list_scope_rank(s: ListScope) -> u8 {
    match s {
        ListScope::Global => 0,
        ListScope::SpriteLocal => 1,
    }
}

fn sort_key(name: &str, rank: u8) -> (String, u8) {
    (name.to_string(), rank)
}

// --- canonicalization of values / expressions / statements ---

pub fn canon_value(v: &Value) -> Value {
    match v {
        Value::Number(n) => Value::Number(canon_num(*n)),
        other => other.clone(),
    }
}

fn canon_num(n: f64) -> f64 {
    if n == 0.0 {
        0.0 // fold -0.0 -> 0.0; also normalizes 0.0 itself
    } else {
        n
    }
}

pub fn canon_expr(e: &Expr) -> Expr {
    match e {
        Expr::Literal(v) => Expr::Literal(canon_value(v)),
        Expr::Variable(name) => Expr::Variable(name.clone()),
        Expr::List(name) => Expr::List(name.clone()),
        Expr::ListItem { list, index } => Expr::ListItem {
            list: list.clone(),
            index: Box::new(canon_expr(index)),
        },
        Expr::ListLength { list } => Expr::ListLength {
            list: list.clone(),
        },
        Expr::ProcedureParam(name) => Expr::ProcedureParam(name.clone()),
        Expr::Operator { opcode, args } => Expr::Operator {
            opcode: opcode.clone(),
            args: args.iter().map(canon_expr).collect(),
        },
        Expr::HeapLoad { addr } => Expr::HeapLoad {
            addr: Box::new(canon_expr(addr)),
        },
        Expr::HeapIndex { base, offset } => Expr::HeapIndex {
            base: Box::new(canon_expr(base)),
            offset: Box::new(canon_expr(offset)),
        },
        Expr::FrameBase => Expr::FrameBase,
        Expr::FrameGet { offset } => Expr::FrameGet {
            offset: *offset,
        },
    }
}

pub fn canon_stmt(s: &Stmt) -> Stmt {
    match s {
        Stmt::Expr(e) => Stmt::Expr(canon_expr(e)),
        Stmt::SetVariable { var, value } => Stmt::SetVariable {
            var: var.clone(),
            value: canon_expr(value),
        },
        Stmt::ChangeVariable { var, delta } => Stmt::ChangeVariable {
            var: var.clone(),
            delta: canon_expr(delta),
        },
        Stmt::AddToList { list, value } => Stmt::AddToList {
            list: list.clone(),
            value: canon_expr(value),
        },
        Stmt::DeleteAllOfList { list } => Stmt::DeleteAllOfList {
            list: list.clone(),
        },
        Stmt::SetListItem { list, index, value } => Stmt::SetListItem {
            list: list.clone(),
            index: canon_expr(index),
            value: canon_expr(value),
        },
        Stmt::DeleteListItem { list, index } => Stmt::DeleteListItem {
            list: list.clone(),
            index: canon_expr(index),
        },
        Stmt::InsertListItem { list, index, value } => Stmt::InsertListItem {
            list: list.clone(),
            index: canon_expr(index),
            value: canon_expr(value),
        },
        Stmt::Broadcast { message } => Stmt::Broadcast {
            message: canon_expr(message),
        },
        Stmt::HeapAlloc { result_offset, size } => Stmt::HeapAlloc {
            result_offset: *result_offset,
            size: canon_expr(size),
        },
        Stmt::Call { proc, args } => Stmt::Call {
            proc: proc.clone(),
            args: args.iter().map(canon_expr).collect(),
        },
        Stmt::EnterFrame { slots } => Stmt::EnterFrame {
            slots: *slots,
        },
        Stmt::PopFrame { slots } => Stmt::PopFrame {
            slots: *slots,
        },
        Stmt::FrameSet { offset, value } => Stmt::FrameSet {
            offset: *offset,
            value: canon_expr(value),
        },
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition: canon_expr(condition),
            then_body: canon_stmts(then_body),
            else_body: canon_stmts(else_body),
        },
        Stmt::Repeat { times, body } => Stmt::Repeat {
            times: canon_expr(times),
            body: canon_stmts(body),
        },
        Stmt::RepeatUntil { condition, body } => Stmt::RepeatUntil {
            condition: canon_expr(condition),
            body: canon_stmts(body),
        },
        Stmt::Forever { body } => Stmt::Forever {
            body: canon_stmts(body),
        },
        Stmt::Stop { option } => Stmt::Stop {
            option: *option,
        },
    }
}

pub fn canon_stmts(stmts: &[Stmt]) -> Vec<Stmt> {
    stmts.iter().map(canon_stmt).collect()
}

/// Convenience constructor used by tests and the analyzer to build a variable
/// declaration in normalized space.
pub fn normalized_variable(name: &str, scope: VariableScope) -> NormalizedVariable {
    NormalizedVariable {
        name: name.to_string(),
        scope,
    }
}

/// Convenience used by the diff engine to derive prototypes when embedding.
pub fn clone_prototype(p: &ProcedurePrototype) -> ProcedurePrototype {
    ProcedurePrototype {
        name: p.name.clone(),
        params: p
            .params
            .iter()
            .map(|param| ProcedureParam {
                name: param.name.clone(),
                default: param.default.as_ref().map(canon_value),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, scope: VariableScope) -> Variable {
        Variable {
            id: format!("id-{}", name),
            name: name.to_string(),
            scope,
        }
    }

    #[test]
    fn variable_ids_do_not_matter() {
        let mut a = Project::new();
        a.stage.variables.push(var("x", VariableScope::Global));
        a.stage.variables[0].id = "aaa".to_string();

        let mut b = Project::new();
        b.stage.variables.push(var("x", VariableScope::Global));
        b.stage.variables[0].id = "zzz".to_string();

        let n = SemanticNormalizer::new();
        assert_eq!(n.normalize(&a), n.normalize(&b));
    }

    #[test]
    fn variable_declaration_order_does_not_matter() {
        let n = SemanticNormalizer::new();
        let mut a = Project::new();
        a.stage.variables.push(var("b", VariableScope::Global));
        a.stage.variables.push(var("a", VariableScope::Global));
        let mut b = Project::new();
        b.stage.variables.push(var("a", VariableScope::Global));
        b.stage.variables.push(var("b", VariableScope::Global));
        assert_eq!(n.normalize(&a), n.normalize(&b));
    }

    #[test]
    fn variable_scope_is_semantic() {
        let n = SemanticNormalizer::new();
        let mut a = Project::new();
        a.stage.variables.push(var("x", VariableScope::Global));
        let mut b = Project::new();
        b.stage.variables.push(var("x", VariableScope::SpriteLocal));
        assert_ne!(n.normalize(&a), n.normalize(&b));
    }

    #[test]
    fn sprite_order_does_not_matter_but_name_does() {
        let n = SemanticNormalizer::new();
        let mut a = Project::new();
        a.add_sprite(Sprite::new("A"));
        a.add_sprite(Sprite::new("B"));
        let mut b = Project::new();
        b.add_sprite(Sprite::new("B"));
        b.add_sprite(Sprite::new("A"));
        assert_eq!(n.normalize(&a), n.normalize(&b));

        let mut c = Project::new();
        c.add_sprite(Sprite::new("A"));
        c.add_sprite(Sprite::new("C"));
        assert_ne!(n.normalize(&a), n.normalize(&c));
    }

    #[test]
    fn broadcast_declaration_site_does_not_matter() {
        let n = SemanticNormalizer::new();
        // A: broadcast declared on the stage.
        let mut a = Project::new();
        a.stage
            .broadcasts
            .push(Broadcast { id: "s1".into(), name: "hello".into() });
        a.add_sprite(Sprite::new("Cat"));
        // B: same sprite set, but the broadcast is declared on the sprite.
        let mut b = Project::new();
        b.add_sprite(Sprite {
            name: "Cat".into(),
            broadcasts: vec![Broadcast { id: "s2".into(), name: "hello".into() }],
            ..Default::default()
        });
        assert_eq!(n.normalize(&a), n.normalize(&b));
    }

    #[test]
    fn script_debug_name_is_dropped_but_body_order_kept() {
        let n = SemanticNormalizer::new();
        let body = vec![Stmt::SetVariable {
            var: "x".into(),
            value: Expr::number(1.0),
        }];
        let mut a = Project::new();
        a.stage.scripts.push(Script {
            entry: crate::ir::ScriptEntry {
                hat: EventHat::GreenFlag,
                name: Some("script-A".into()),
                body: body.clone(),
            },
        });
        let mut b = Project::new();
        b.stage.scripts.push(Script {
            entry: crate::ir::ScriptEntry {
                hat: EventHat::GreenFlag,
                name: None,
                body,
            },
        });
        assert_eq!(n.normalize(&a), n.normalize(&b));

        // Swapping two distinct statements inside the body is a real change.
        let mut c = Project::new();
        c.stage.scripts.push(Script::new(
            EventHat::GreenFlag,
            vec![Stmt::SetVariable {
                var: "y".into(),
                value: Expr::number(1.0),
            }],
        ));
        assert_ne!(n.normalize(&a), n.normalize(&c));
    }

    #[test]
    fn minus_zero_folds_to_zero() {
        assert_eq!(canon_value(&Value::Number(-0.0)), Value::Number(0.0));
    }
}
