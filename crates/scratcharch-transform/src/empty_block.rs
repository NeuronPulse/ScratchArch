use scratcharch_scratchgraph::ir::{Project, Stmt};

use crate::pass::{PassReport, TransformPass};

#[derive(Default)]
pub struct EmptyBlockRemoval;

impl EmptyBlockRemoval {
    pub fn new() -> Self {
        Self
    }

    /// Remove empty nested blocks. Returns count of removals.
    fn visit_body(body: &mut Vec<Stmt>) -> u32 {
        let mut count = 0;
        let mut i = 0;
        while i < body.len() {
            let removed = match &mut body[i] {
                Stmt::If {
                    ref mut then_body,
                    ref mut else_body,
                    ..
                } => {
                    count += Self::visit_body(then_body);
                    count += Self::visit_body(else_body);
                    then_body.is_empty() && else_body.is_empty()
                }
                Stmt::Repeat { body: ref mut b, .. }
                | Stmt::RepeatUntil { body: ref mut b, .. }
                | Stmt::Forever { body: ref mut b } => {
                    count += Self::visit_body(b);
                    b.is_empty()
                }
                _ => false,
            };

            if removed {
                body.remove(i);
                count += 1;
            } else {
                i += 1;
            }
        }
        count
    }
}

impl TransformPass for EmptyBlockRemoval {
    fn name(&self) -> &str {
        "Empty Block Removal"
    }

    fn run(&mut self, project: &mut Project) -> PassReport {
        // Removing an empty `If`/`Repeat` is always sound. Removing an empty
        // `RepeatUntil`/`Forever` is sound only because conditions and bodies
        // in ScratchGraph are pure (no side effects, no timing/concurrency in
        // the IR), so an empty loop is unobservable except as an idle spin.
        let mut total = 0;

        for script in &mut project.stage.scripts {
            total += Self::visit_body(&mut script.entry.body);
        }
        for proc in &mut project.stage.procedures {
            total += Self::visit_body(&mut proc.body);
        }
        for sprite in &mut project.sprites {
            for script in &mut sprite.scripts {
                total += Self::visit_body(&mut script.entry.body);
            }
            for proc in &mut sprite.procedures {
                total += Self::visit_body(&mut proc.body);
            }
        }

        PassReport {
            name: self.name().to_string(),
            modifications: total,
            deleted_nodes: total,
            saved_variables: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scratcharch_scratchgraph::ir::*;

    #[test]
    fn test_empty_block_removal() {
        let mut project = Project {
            stage: Stage {
                name: "Stage".into(),
                variables: vec![],
                lists: vec![],
                broadcasts: vec![],
                scripts: vec![Script::new(
                    EventHat::KeyPressed("space".into()),
                    vec![Stmt::If {
                        condition: Expr::Literal(Value::Bool(true)),
                        then_body: vec![],
                        else_body: vec![],
                    }],
                )],
                procedures: vec![],
                costumes: vec![],
                sounds: vec![],
            },
            sprites: vec![],
        };
        let before = project.stage.scripts.len();

        let mut pass = EmptyBlockRemoval::new();
        let report = pass.run(&mut project);

        assert!(report.deleted_nodes > 0, "should remove empty if blocks");
        assert_eq!(
            project.stage.scripts.len(),
            before,
            "script count should not change (only body content removed)"
        );
        assert!(
            project.stage.scripts[0].entry.body.is_empty(),
            "if block should be removed from body"
        );
    }
}
