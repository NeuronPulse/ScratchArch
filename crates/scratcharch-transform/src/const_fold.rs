use scratcharch_scratchgraph::ir::{Expr, Project, Stmt, Value};

use crate::pass::{PassReport, TransformPass};

#[derive(Default)]
pub struct ConstantFolding;

impl ConstantFolding {
    pub fn new() -> Self {
        Self
    }

    /// Fold an expression tree bottom-up. Returns true if anything changed.
    fn fold_expr(expr: &mut Expr) -> bool {
        match expr {
            Expr::Operator { args, .. } => {
                let mut changed = false;
                for arg in args.iter_mut() {
                    changed |= Self::fold_expr(arg);
                }
                changed |= Self::try_fold(expr);
                changed
            }
            Expr::ListItem { index, .. } => Self::fold_expr(index),
            Expr::HeapLoad { addr } => Self::fold_expr(addr),
            Expr::HeapIndex { base, offset } => {
                Self::fold_expr(base) | Self::fold_expr(offset)
            }
            _ => false,
        }
    }

    /// Fold a single operator whose operands are all numeric literals.
    ///
    /// Only numeric arithmetic is folded: the Scratch runtime and Rust both
    /// compute IEEE-754 doubles, so replacing `10 + 20` with `30` cannot change
    /// the result. Comparison (`operator_equals`/`operator_lt`/`operator_gt`)
    /// and boolean (`operator_not`) operators are intentionally left unfolded:
    /// they produce booleans, which are not interchangeable with a numeric
    /// literal in the ScratchGraph value model. Division by zero is left
    /// unfolded so the runtime semantics are preserved exactly.
    fn try_fold(expr: &mut Expr) -> bool {
        let (opcode, args) = match expr {
            Expr::Operator { opcode, args } => (opcode.clone(), args),
            _ => return false,
        };

        let mut numbers = Vec::with_capacity(args.len());
        for arg in args.iter() {
            match arg {
                Expr::Literal(Value::Number(n)) => numbers.push(*n),
                _ => return false,
            }
        }

        if numbers.len() < 2 {
            return false;
        }

        let result = match opcode.as_str() {
            "operator_add" => Some(numbers.iter().sum()),
            "operator_subtract" => Some(numbers[0] - numbers[1]),
            "operator_multiply" => Some(numbers.iter().product()),
            "operator_divide" => {
                if numbers[1] != 0.0 {
                    Some(numbers[0] / numbers[1])
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(value) = result {
            *expr = Expr::Literal(Value::Number(value));
            true
        } else {
            false
        }
    }

    fn fold_stmt(stmt: &mut Stmt) -> bool {
        match stmt {
            Stmt::SetVariable { value, .. } => Self::fold_expr(value),
            Stmt::ChangeVariable { delta, .. } => Self::fold_expr(delta),
            Stmt::AddToList { value, .. } => Self::fold_expr(value),
            Stmt::SetListItem { index, value, .. } => {
                Self::fold_expr(index) | Self::fold_expr(value)
            }
            Stmt::DeleteListItem { index, .. } => Self::fold_expr(index),
            Stmt::InsertListItem { index, value, .. } => {
                Self::fold_expr(index) | Self::fold_expr(value)
            }
            Stmt::Broadcast { message } => Self::fold_expr(message),
            Stmt::HeapAlloc { size, .. } => Self::fold_expr(size),
            Stmt::Call { args, .. } => {
                let mut c = false;
                for a in args.iter_mut() {
                    c |= Self::fold_expr(a);
                }
                c
            }
            Stmt::FrameSet { value, .. } => Self::fold_expr(value),
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let mut c = Self::fold_expr(condition);
                for s in then_body.iter_mut() {
                    c |= Self::fold_stmt(s);
                }
                for s in else_body.iter_mut() {
                    c |= Self::fold_stmt(s);
                }
                c
            }
            Stmt::Repeat { times, body } => {
                let mut c = Self::fold_expr(times);
                for s in body.iter_mut() {
                    c |= Self::fold_stmt(s);
                }
                c
            }
            Stmt::RepeatUntil { condition, body } => {
                let mut c = Self::fold_expr(condition);
                for s in body.iter_mut() {
                    c |= Self::fold_stmt(s);
                }
                c
            }
            Stmt::Forever { body } => {
                let mut c = false;
                for s in body.iter_mut() {
                    c |= Self::fold_stmt(s);
                }
                c
            }
            Stmt::Expr(e) => Self::fold_expr(e),
            _ => false,
        }
    }

    fn count_folded(project: &mut Project) -> u32 {
        let mut count = 0u32;
        for s in &mut project.stage.scripts {
            for stmt in &mut s.entry.body {
                if Self::fold_stmt(stmt) {
                    count += 1;
                }
            }
        }
        for p in &mut project.stage.procedures {
            for stmt in &mut p.body {
                if Self::fold_stmt(stmt) {
                    count += 1;
                }
            }
        }
        for sprite in &mut project.sprites {
            for s in &mut sprite.scripts {
                for stmt in &mut s.entry.body {
                    if Self::fold_stmt(stmt) {
                        count += 1;
                    }
                }
            }
            for p in &mut sprite.procedures {
                for stmt in &mut p.body {
                    if Self::fold_stmt(stmt) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

impl TransformPass for ConstantFolding {
    fn name(&self) -> &str {
        "Constant Folding"
    }

    fn run(&mut self, project: &mut Project) -> PassReport {
        let modifications = Self::count_folded(project);
        PassReport {
            name: self.name().to_string(),
            modifications,
            deleted_nodes: 0,
            saved_variables: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scratcharch_scratchgraph::ir::*;

    fn stage_with_body(body: Vec<Stmt>) -> Stage {
        Stage {
            name: "Stage".into(),
            variables: vec![Variable::new("x_id", "x")],
            lists: vec![],
            broadcasts: vec![],
            scripts: vec![Script::new(EventHat::GreenFlag, body)],
            procedures: vec![],
            costumes: vec![],
            sounds: vec![],
        }
    }

    #[test]
    fn test_constant_folding() {
        let mut project = Project {
            stage: stage_with_body(vec![Stmt::SetVariable {
                var: "x".into(),
                value: Expr::Operator {
                    opcode: "operator_add".into(),
                    args: vec![
                        Expr::Literal(Value::Number(1.0)),
                        Expr::Literal(Value::Number(2.0)),
                    ],
                },
            }]),
            sprites: vec![],
        };
        let mut pass = ConstantFolding::new();
        let report = pass.run(&mut project);

        assert!(report.modifications > 0, "should fold expressions");

        if let Stmt::SetVariable { value, .. } = &project.stage.scripts[0].entry.body[0] {
            assert_eq!(
                value,
                &Expr::Literal(Value::Number(3.0)),
                "1+2 should fold to 3"
            );
        } else {
            panic!("expected SetVariable");
        }
    }

    #[test]
    fn test_comparisons_are_not_folded() {
        // operator_equals yields a boolean, which must not be replaced by a
        // numeric literal.
        let mut project = Project {
            stage: stage_with_body(vec![Stmt::SetVariable {
                var: "x".into(),
                value: Expr::Operator {
                    opcode: "operator_equals".into(),
                    args: vec![
                        Expr::Literal(Value::Number(1.0)),
                        Expr::Literal(Value::Number(1.0)),
                    ],
                },
            }]),
            sprites: vec![],
        };
        let mut pass = ConstantFolding::new();
        let report = pass.run(&mut project);

        assert_eq!(report.modifications, 0, "comparisons must not be folded");
        assert!(
            matches!(
                &project.stage.scripts[0].entry.body[0],
                Stmt::SetVariable {
                    value: Expr::Operator { opcode, .. },
                    ..
                } if opcode == "operator_equals"
            ),
            "operator_equals must be preserved"
        );
    }

    #[test]
    fn test_division_by_zero_not_folded() {
        let mut project = Project {
            stage: stage_with_body(vec![Stmt::SetVariable {
                var: "x".into(),
                value: Expr::Operator {
                    opcode: "operator_divide".into(),
                    args: vec![
                        Expr::Literal(Value::Number(1.0)),
                        Expr::Literal(Value::Number(0.0)),
                    ],
                },
            }]),
            sprites: vec![],
        };
        let mut pass = ConstantFolding::new();
        let report = pass.run(&mut project);

        assert_eq!(report.modifications, 0, "division by zero must not be folded");
        assert!(
            matches!(
                &project.stage.scripts[0].entry.body[0],
                Stmt::SetVariable {
                    value: Expr::Operator { opcode, .. },
                    ..
                } if opcode == "operator_divide"
            ),
            "division by zero must be preserved"
        );
    }

    #[test]
    fn test_chained_folding() {
        // Folding runs bottom-up so `(10 + 10) + 5` collapses to a single
        // literal in one pass.
        let mut project = Project {
            stage: stage_with_body(vec![Stmt::SetVariable {
                var: "x".into(),
                value: Expr::operator(
                    "operator_add",
                    vec![
                        Expr::operator(
                            "operator_add",
                            vec![Expr::number(10.0), Expr::number(10.0)],
                        ),
                        Expr::number(5.0),
                    ],
                ),
            }]),
            sprites: vec![],
        };
        let mut pass = ConstantFolding::new();
        let report = pass.run(&mut project);

        assert_eq!(report.modifications, 1, "one statement changed");
        assert_eq!(
            project.stage.scripts[0].entry.body[0],
            Stmt::SetVariable {
                var: "x".into(),
                value: Expr::Literal(Value::Number(25.0)),
            }
        );
    }
}
