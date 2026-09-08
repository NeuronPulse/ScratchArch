use std::collections::HashSet;

use scratcharch_scratchgraph::ir::{EventHat, Expr, Procedure, Project, Script, Stmt, Value};

use crate::pass::{PassReport, TransformPass};

#[derive(Default)]
pub struct DeadScriptElimination;

impl DeadScriptElimination {
    pub fn new() -> Self {
        Self
    }

    /// Collect procedure names called from a list of statements.
    fn collect_calls(stmts: &[Stmt], out: &mut HashSet<String>) {
        for stmt in stmts {
            match stmt {
                Stmt::Call { proc, .. } => {
                    out.insert(proc.clone());
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    Self::collect_calls(then_body, out);
                    Self::collect_calls(else_body, out);
                }
                Stmt::Repeat { body: b, .. }
                | Stmt::RepeatUntil { body: b, .. }
                | Stmt::Forever { body: b } => Self::collect_calls(b, out),
                _ => {}
            }
        }
    }

    /// Collect procedure names referenced within one target.
    ///
    /// Custom blocks are defined per target in Scratch, so a procedure is live
    /// only if the target that owns it calls it. Procedure bodies are included
    /// so that self/mutual recursion keeps a procedure alive (this is
    /// deliberately conservative: a cycle with no external caller is retained).
    fn target_called(scripts: &[Script], procedures: &[Procedure]) -> HashSet<String> {
        let mut called = HashSet::new();
        for script in scripts {
            Self::collect_calls(&script.entry.body, &mut called);
        }
        for proc in procedures {
            Self::collect_calls(&proc.body, &mut called);
        }
        called
    }

    /// Collect the broadcast messages sent from a list of statements.
    ///
    /// When a broadcast message is not a literal string (for example it is read
    /// from a variable) we cannot tell which receive hats it can target, so
    /// `dynamic` is set and no receive script can be proven unreachable.
    fn collect_broadcasts(stmts: &[Stmt], sent: &mut HashSet<String>, dynamic: &mut bool) {
        for stmt in stmts {
            match stmt {
                Stmt::Broadcast { message } => match message {
                    Expr::Literal(Value::String(name)) => {
                        sent.insert(name.clone());
                    }
                    _ => *dynamic = true,
                },
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    Self::collect_broadcasts(then_body, sent, dynamic);
                    Self::collect_broadcasts(else_body, sent, dynamic);
                }
                Stmt::Repeat { body: b, .. }
                | Stmt::RepeatUntil { body: b, .. }
                | Stmt::Forever { body: b } => Self::collect_broadcasts(b, sent, dynamic),
                _ => {}
            }
        }
    }

    /// Drop scripts whose receive hat can be proven to never fire.
    fn remove_dead_scripts(
        scripts: &mut Vec<Script>,
        sent: &HashSet<String>,
        dynamic: bool,
    ) -> u32 {
        let before = scripts.len() as u32;
        scripts.retain(|script| match &script.entry.hat {
            EventHat::GreenFlag
            | EventHat::KeyPressed(_)
            | EventHat::SpriteClicked
            | EventHat::CloneStart => true,
            EventHat::BroadcastReceived(name) => dynamic || sent.contains(name),
        });
        before - scripts.len() as u32
    }
}

impl TransformPass for DeadScriptElimination {
    fn name(&self) -> &str {
        "Dead Script Elimination"
    }

    fn run(&mut self, project: &mut Project) -> PassReport {
        let mut deleted = 0u32;

        // Broadcast messages are global: a `when I receive` script is live if
        // the message is broadcast anywhere in the project (stage or sprites,
        // scripts or procedures). Collect sends project-wide before pruning.
        let mut sent = HashSet::new();
        let mut dynamic = false;
        for script in &project.stage.scripts {
            Self::collect_broadcasts(&script.entry.body, &mut sent, &mut dynamic);
        }
        for proc in &project.stage.procedures {
            Self::collect_broadcasts(&proc.body, &mut sent, &mut dynamic);
        }
        for sprite in &project.sprites {
            for script in &sprite.scripts {
                Self::collect_broadcasts(&script.entry.body, &mut sent, &mut dynamic);
            }
            for proc in &sprite.procedures {
                Self::collect_broadcasts(&proc.body, &mut sent, &mut dynamic);
            }
        }

        // Stage and sprite scripts.
        deleted += Self::remove_dead_scripts(&mut project.stage.scripts, &sent, dynamic);
        for sprite in &mut project.sprites {
            deleted += Self::remove_dead_scripts(&mut sprite.scripts, &sent, dynamic);
        }

        // Stage procedures: keep those called from within the stage.
        let stage_called = Self::target_called(&project.stage.scripts, &project.stage.procedures);
        let before = project.stage.procedures.len() as u32;
        project
            .stage
            .procedures
            .retain(|proc| stage_called.contains(&proc.prototype.name));
        deleted += before - project.stage.procedures.len() as u32;

        // Sprite procedures: keep those called from within the owning sprite.
        for sprite in &mut project.sprites {
            let called = Self::target_called(&sprite.scripts, &sprite.procedures);
            let before = sprite.procedures.len() as u32;
            sprite
                .procedures
                .retain(|proc| called.contains(&proc.prototype.name));
            deleted += before - sprite.procedures.len() as u32;
        }

        PassReport {
            name: self.name().to_string(),
            modifications: deleted,
            deleted_nodes: deleted,
            saved_variables: 0,
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
                variables: vec![Variable::new("x_id", "x")],
                lists: vec![],
                broadcasts: vec![],
                scripts: vec![
                    Script::new(
                        EventHat::GreenFlag,
                        vec![Stmt::Call {
                            proc: "my_proc".into(),
                            args: vec![],
                        }],
                    ),
                    Script::new(EventHat::BroadcastReceived("unused_ev".into()), vec![]),
                ],
                procedures: vec![Procedure::new("my_proc", vec![], vec![])],
                costumes: vec![],
                sounds: vec![],
            },
            sprites: vec![],
        }
    }

    #[test]
    fn test_dead_script_elimination() {
        let mut project = make_project();
        let mut pass = DeadScriptElimination::new();
        let report = pass.run(&mut project);

        assert!(
            report.deleted_nodes > 0,
            "should remove unreferenced broadcast script"
        );
        assert_eq!(
            project.stage.scripts.len(),
            1,
            "only one script should remain (green flag)"
        );
        assert_eq!(
            project.stage.procedures.len(),
            1,
            "called procedure should remain"
        );
    }

    fn stage(name: &str, scripts: Vec<Script>, procedures: Vec<Procedure>) -> Stage {
        Stage {
            name: name.into(),
            variables: vec![],
            lists: vec![],
            broadcasts: vec![],
            scripts,
            procedures,
            costumes: vec![],
            sounds: vec![],
        }
    }

    fn sprite(name: &str, scripts: Vec<Script>, procedures: Vec<Procedure>) -> Sprite {
        Sprite {
            name: name.into(),
            variables: vec![],
            lists: vec![],
            broadcasts: vec![],
            scripts,
            procedures,
            costumes: vec![],
            sounds: vec![],
        }
    }

    fn send_broadcast(message: Expr) -> Stmt {
        Stmt::Broadcast { message }
    }

    #[test]
    fn test_cross_target_broadcast_keeps_receiver() {
        // Broadcasts are global: a sprite that broadcasts "go" keeps the stage
        // script that receives "go".
        let mut project = Project {
            stage: stage(
                "Stage",
                vec![Script::new(EventHat::BroadcastReceived("go".into()), vec![])],
                vec![],
            ),
            sprites: vec![sprite(
                "Sprite1",
                vec![Script::new(
                    EventHat::GreenFlag,
                    vec![send_broadcast(Expr::string("go"))],
                )],
                vec![],
            )],
        };

        let mut pass = DeadScriptElimination::new();
        let report = pass.run(&mut project);

        assert_eq!(
            project.stage.scripts.len(),
            1,
            "stage receiver of a sprite broadcast must stay"
        );
        assert_eq!(
            report.deleted_nodes,
            0,
            "no live script should be removed"
        );
    }

    #[test]
    fn test_procedure_broadcast_keeps_receiver() {
        // Broadcasts sent from inside a procedure body also reach receive hats.
        let mut project = Project {
            stage: stage(
                "Stage",
                vec![
                    Script::new(
                        EventHat::GreenFlag,
                        vec![Stmt::Call {
                            proc: "sender".into(),
                            args: vec![],
                        }],
                    ),
                    Script::new(EventHat::BroadcastReceived("go".into()), vec![]),
                ],
                vec![Procedure::new(
                    "sender",
                    vec![],
                    vec![send_broadcast(Expr::string("go"))],
                )],
            ),
            sprites: vec![],
        };

        let mut pass = DeadScriptElimination::new();
        let report = pass.run(&mut project);

        assert_eq!(project.stage.scripts.len(), 2, "both scripts must stay");
        assert_eq!(
            project.stage.procedures.len(),
            1,
            "broadcasting procedure must stay"
        );
        assert_eq!(report.deleted_nodes, 0, "no live script should be removed");
    }

    #[test]
    fn test_dynamic_broadcast_keeps_receivers() {
        // A broadcast whose message is computed at runtime could target any
        // receive hat, so no receiver can be proven dead.
        let mut project = Project {
            stage: stage(
                "Stage",
                vec![
                    Script::new(
                        EventHat::GreenFlag,
                        vec![send_broadcast(Expr::variable("msg"))],
                    ),
                    Script::new(EventHat::BroadcastReceived("anything".into()), vec![]),
                ],
                vec![],
            ),
            sprites: vec![],
        };

        let mut pass = DeadScriptElimination::new();
        let report = pass.run(&mut project);

        assert_eq!(
            project.stage.scripts.len(),
            2,
            "dynamic broadcast keeps all receivers"
        );
        assert_eq!(report.deleted_nodes, 0, "no live script should be removed");
    }

    #[test]
    fn test_procedure_scope_isolation() {
        // Custom blocks are per-target: a stage procedure called from the stage
        // must stay even though an identically named sprite procedure is dead.
        let mut project = Project {
            stage: stage(
                "Stage",
                vec![Script::new(
                    EventHat::GreenFlag,
                    vec![Stmt::Call {
                        proc: "util".into(),
                        args: vec![],
                    }],
                )],
                vec![Procedure::new("util", vec![], vec![])],
            ),
            sprites: vec![sprite(
                "Sprite1",
                vec![Script::new(EventHat::GreenFlag, vec![])],
                vec![Procedure::new("util", vec![], vec![])],
            )],
        };

        let mut pass = DeadScriptElimination::new();
        pass.run(&mut project);

        assert_eq!(
            project.stage.procedures.len(),
            1,
            "stage procedure is called from the stage"
        );
        assert_eq!(
            project.sprites[0].procedures.len(),
            0,
            "sprite procedure is never called by its own sprite"
        );
    }
}
