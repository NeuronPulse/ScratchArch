use std::collections::HashSet;

use scratcharch_scratchgraph::ir::{Expr, Project, Stmt};

use crate::pass::{PassReport, TransformPass};

#[derive(Default)]
pub struct VariableAnalysis;

impl VariableAnalysis {
    pub fn new() -> Self {
        Self
    }

    fn scan_usage(stmts: &[Stmt], used: &mut HashSet<String>) {
        for stmt in stmts {
            match stmt {
                Stmt::SetVariable { var, value } => {
                    used.insert(var.clone());
                    Self::expr_vars(value, used);
                }
                Stmt::ChangeVariable { var, delta } => {
                    used.insert(var.clone());
                    Self::expr_vars(delta, used);
                }
                Stmt::AddToList { value, .. } => Self::expr_vars(value, used),
                Stmt::SetListItem { index, value, .. } => {
                    Self::expr_vars(index, used);
                    Self::expr_vars(value, used);
                }
                Stmt::DeleteListItem { index, .. } => Self::expr_vars(index, used),
                Stmt::InsertListItem { index, value, .. } => {
                    Self::expr_vars(index, used);
                    Self::expr_vars(value, used);
                }
                Stmt::Broadcast { message } => Self::expr_vars(message, used),
                Stmt::Call { args, .. } => {
                    for a in args {
                        Self::expr_vars(a, used);
                    }
                }
                Stmt::FrameSet { value, .. } => Self::expr_vars(value, used),
                Stmt::HeapAlloc { size, .. } => Self::expr_vars(size, used),
                Stmt::Expr(e) => Self::expr_vars(e, used),
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    Self::expr_vars(condition, used);
                    Self::scan_usage(then_body, used);
                    Self::scan_usage(else_body, used);
                }
                Stmt::Repeat { times, body: b } => {
                    Self::expr_vars(times, used);
                    Self::scan_usage(b, used);
                }
                Stmt::RepeatUntil { condition, body: b } => {
                    Self::expr_vars(condition, used);
                    Self::scan_usage(b, used);
                }
                Stmt::Forever { body: b } => Self::scan_usage(b, used),
                _ => {}
            }
        }
    }

    fn expr_vars(expr: &Expr, used: &mut HashSet<String>) {
        match expr {
            Expr::Variable(name) => {
                used.insert(name.clone());
            }
            Expr::ProcedureParam(_)
            | Expr::List(_)
            | Expr::ListLength { .. }
            | Expr::FrameBase => {}
            Expr::Literal(_) => {}
            Expr::ListItem { index, .. } => Self::expr_vars(index, used),
            Expr::Operator { args, .. } => {
                for a in args {
                    Self::expr_vars(a, used);
                }
            }
            Expr::HeapLoad { addr } => Self::expr_vars(addr, used),
            Expr::HeapIndex { base, offset } => {
                Self::expr_vars(base, used);
                Self::expr_vars(offset, used);
            }
            Expr::FrameGet { .. } => {}
        }
    }
}

impl TransformPass for VariableAnalysis {
    fn name(&self) -> &str {
        "Variable Analysis"
    }

    fn run(&mut self, project: &mut Project) -> PassReport {
        // Collect used variable names from all scripts and procedures.
        //
        // This is deliberately conservative with respect to scope: variable
        // declarations may be global (owned by the stage) or sprite-local, but
        // references resolve by name. Because we only delete a declared
        // variable when its name is referenced *nowhere* in the project, a
        // scope mix-up can at most over-retain a variable, never delete a live
        // one. Writes count as usage, so write-only variables are kept too.
        let mut used = HashSet::new();
        for s in &project.stage.scripts {
            Self::scan_usage(&s.entry.body, &mut used);
        }
        for p in &project.stage.procedures {
            Self::scan_usage(&p.body, &mut used);
        }
        for sprite in &project.sprites {
            for s in &sprite.scripts {
                Self::scan_usage(&s.entry.body, &mut used);
            }
            for p in &sprite.procedures {
                Self::scan_usage(&p.body, &mut used);
            }
        }

        let mut saved = 0u32;

        // Stage variables
        let before = project.stage.variables.len() as u32;
        project.stage.variables.retain(|v| used.contains(&v.name));
        saved += before - project.stage.variables.len() as u32;

        // Sprite variables
        for sprite in &mut project.sprites {
            let before = sprite.variables.len() as u32;
            sprite.variables.retain(|v| used.contains(&v.name));
            saved += before - sprite.variables.len() as u32;
        }

        PassReport {
            name: self.name().to_string(),
            modifications: saved,
            deleted_nodes: 0,
            saved_variables: saved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scratcharch_scratchgraph::ir::*;

    fn make_project() -> Project {
        Project {
            stage: Stage {
                name: "Stage".into(),
                variables: vec![
                    Variable::new("x_id", "x"),
                    Variable::new("unused_id", "unused"),
                ],
                lists: vec![],
                broadcasts: vec![],
                scripts: vec![Script::new(
                    EventHat::GreenFlag,
                    vec![Stmt::SetVariable {
                        var: "x".into(),
                        value: Expr::Literal(Value::Number(1.0)),
                    }],
                )],
                procedures: vec![],
                costumes: vec![],
                sounds: vec![],
            },
            sprites: vec![],
        }
    }

    #[test]
    fn test_variable_analysis() {
        let mut project = make_project();
        let mut pass = VariableAnalysis::new();
        let report = pass.run(&mut project);

        assert!(report.saved_variables > 0, "should remove unused variables");
        let names: Vec<&str> = project
            .stage
            .variables
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        assert!(names.contains(&"x"), "x should remain (used)");
        assert!(!names.contains(&"unused"), "unused should be removed");
    }
}
