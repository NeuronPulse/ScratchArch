//! Script reachability analysis.
//!
//! A script is reachable if its event hat can fire in normal Scratch execution.
//! Green-flag scripts are always reachable. Other scripts are reachable if the
//! project contains a broadcast sender, key handler, sprite click, or clone
//! trigger that matches their hat. Static analysis can only approximate dynamic
//! dispatch; this module reports scripts that are *definitely* unreachable from
//! static evidence.

use scratcharch_scratchgraph::ir::{EventHat, Expr, Project, Sprite, Stage, Stmt, Value as SgValue};

/// A script that the analysis determined is unreachable.
#[derive(Debug, Clone, PartialEq)]
pub struct UnreachableScript {
    pub target_name: String,
    pub script_index: usize,
    pub hat: EventHat,
}

/// Reachability analysis.
#[derive(Debug, Clone, Default)]
pub struct ReachabilityAnalysis;

impl ReachabilityAnalysis {
    pub fn new() -> Self {
        Self
    }

    /// Analyze a project and return all unreachable scripts.
    pub fn analyze(&self, project: &Project) -> Vec<UnreachableScript> {
        let mut unreachable = Vec::new();
        self.analyze_stage(&project.stage, &mut unreachable);
        for sprite in &project.sprites {
            self.analyze_sprite(sprite, &mut unreachable);
        }
        unreachable
    }

    fn analyze_stage(&self, stage: &Stage, out: &mut Vec<UnreachableScript>) {
        for (i, script) in stage.scripts.iter().enumerate() {
            if !self.is_reachable(&script.entry.hat, stage, None) {
                out.push(UnreachableScript {
                    target_name: stage.name.clone(),
                    script_index: i,
                    hat: script.entry.hat.clone(),
                });
            }
        }
    }

    fn analyze_sprite(&self, sprite: &Sprite, out: &mut Vec<UnreachableScript>) {
        for (i, script) in sprite.scripts.iter().enumerate() {
            if !self.is_reachable(&script.entry.hat, &empty_stage(&sprite.name), Some(sprite)) {
                out.push(UnreachableScript {
                    target_name: sprite.name.clone(),
                    script_index: i,
                    hat: script.entry.hat.clone(),
                });
            }
        }
    }

    fn is_reachable(&self, hat: &EventHat, stage: &Stage, sprite: Option<&Sprite>) -> bool {
        match hat {
            EventHat::GreenFlag => true,
            EventHat::KeyPressed(_) => true,
            EventHat::SpriteClicked => sprite.is_some(),
            EventHat::BroadcastReceived(name) => {
                self.broadcast_used(name, stage) || sprite.map(|s| self.broadcast_used_in_sprite(name, s)).unwrap_or(false)
            }
            EventHat::CloneStart => sprite.is_some(),
        }
    }

    fn broadcast_used(&self, name: &str, stage: &Stage) -> bool {
        stage.scripts.iter().any(|s| self.body_sends_broadcast(&s.entry.body, name))
            || stage.procedures.iter().any(|p| self.body_sends_broadcast(&p.body, name))
    }

    fn broadcast_used_in_sprite(&self, name: &str, sprite: &Sprite) -> bool {
        sprite.scripts.iter().any(|s| self.body_sends_broadcast(&s.entry.body, name))
            || sprite.procedures.iter().any(|p| self.body_sends_broadcast(&p.body, name))
    }

    fn body_sends_broadcast(&self, body: &[Stmt], name: &str) -> bool {
        body.iter().any(|stmt| self.stmt_sends_broadcast(stmt, name))
    }

    fn stmt_sends_broadcast(&self, stmt: &Stmt, name: &str) -> bool {
        match stmt {
            Stmt::Broadcast { message } => {
                matches!(message, Expr::Literal(SgValue::String(s)) if s == name)
            }
            Stmt::If { then_body, else_body, .. } => {
                self.body_sends_broadcast(then_body, name) || self.body_sends_broadcast(else_body, name)
            }
            Stmt::Repeat { body: b, .. }
            | Stmt::RepeatUntil { body: b, .. }
            | Stmt::Forever { body: b } => self.body_sends_broadcast(b, name),
            _ => false,
        }
    }
}

fn empty_stage(name: &str) -> Stage {
    Stage {
        name: name.to_string(),
        variables: Vec::new(),
        lists: Vec::new(),
        broadcasts: Vec::new(),
        costumes: Vec::new(),
        sounds: Vec::new(),
        scripts: Vec::new(),
        procedures: Vec::new(),
    }
}
