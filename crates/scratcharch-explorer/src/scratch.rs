use serde::Serialize;

use scratcharch_scratchgraph::ir::Project;

/// Summary of a ScratchGraph project for exploration tools.
#[derive(Debug, Clone, Serialize)]
pub struct ScratchProjectSummary {
    pub project_name: String,
    pub sprite_count: usize,
    pub targets: Vec<TargetSummary>,
}

/// Summary of a single Scratch target (stage or sprite).
#[derive(Debug, Clone, Serialize)]
pub struct TargetSummary {
    pub name: String,
    pub is_stage: bool,
    pub script_count: usize,
    pub procedure_count: usize,
    pub variable_count: usize,
    pub list_count: usize,
    pub broadcast_count: usize,
    pub scripts: Vec<ScriptSummary>,
    pub procedures: Vec<ProcedureSummary>,
}

/// Summary of a single Scratch script.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptSummary {
    pub hat: String,
    pub name: Option<String>,
    pub body_length: usize,
}

/// Summary of a single Scratch procedure.
#[derive(Debug, Clone, Serialize)]
pub struct ProcedureSummary {
    pub name: String,
    pub param_count: usize,
    pub body_length: usize,
    pub frame_size: u32,
}

/// Explorer for ScratchGraph projects.
#[derive(Default)]
pub struct ScratchExplorer;

impl ScratchExplorer {
    pub fn new() -> Self {
        Self
    }

    /// Produce a summary of an entire Scratch project.
    pub fn explore_project(&self, project: &Project) -> ScratchProjectSummary {
        let mut targets = Vec::new();

        targets.push(TargetSummary {
            name: project.stage.name.clone(),
            is_stage: true,
            script_count: project.stage.scripts.len(),
            procedure_count: project.stage.procedures.len(),
            variable_count: project.stage.variables.len(),
            list_count: project.stage.lists.len(),
            broadcast_count: project.stage.broadcasts.len(),
            scripts: project
                .stage
                .scripts
                .iter()
                .map(|s| ScriptSummary {
                    hat: format!("{:?}", s.entry.hat),
                    name: s.entry.name.clone(),
                    body_length: s.entry.body.len(),
                })
                .collect(),
            procedures: project
                .stage
                .procedures
                .iter()
                .map(|p| ProcedureSummary {
                    name: p.prototype.name.clone(),
                    param_count: p.prototype.params.len(),
                    body_length: p.body.len(),
                    frame_size: p.frame_size,
                })
                .collect(),
        });

        for sprite in &project.sprites {
            targets.push(TargetSummary {
                name: sprite.name.clone(),
                is_stage: false,
                script_count: sprite.scripts.len(),
                procedure_count: sprite.procedures.len(),
                variable_count: sprite.variables.len(),
                list_count: sprite.lists.len(),
                broadcast_count: 0,
                scripts: sprite
                    .scripts
                    .iter()
                    .map(|s| ScriptSummary {
                        hat: format!("{:?}", s.entry.hat),
                        name: s.entry.name.clone(),
                        body_length: s.entry.body.len(),
                    })
                    .collect(),
                procedures: sprite
                    .procedures
                    .iter()
                    .map(|p| ProcedureSummary {
                        name: p.prototype.name.clone(),
                        param_count: p.prototype.params.len(),
                        body_length: p.body.len(),
                        frame_size: p.frame_size,
                    })
                    .collect(),
            });
        }

        ScratchProjectSummary {
            project_name: project.stage.name.clone(),
            sprite_count: project.sprites.len(),
            targets,
        }
    }

    /// Return a human-readable text report.
    pub fn to_text(&self, project: &Project) -> String {
        let summary = self.explore_project(project);
        let mut out = String::new();
        out.push_str(&format!("Scratch Project: {}\n", summary.project_name));
        out.push_str(&format!("Sprites: {}\n\n", summary.sprite_count));

        for target in &summary.targets {
            let kind = if target.is_stage { "Stage" } else { "Sprite" };
            out.push_str(&format!("--- {}: {} ---\n", kind, target.name));
            out.push_str(&format!("  scripts: {}\n", target.script_count));
            out.push_str(&format!("  procedures: {}\n", target.procedure_count));
            out.push_str(&format!("  variables: {}\n", target.variable_count));
            out.push_str(&format!("  lists: {}\n", target.list_count));
            if target.is_stage {
                out.push_str(&format!("  broadcasts: {}\n", target.broadcast_count));
            }
            for script in &target.scripts {
                out.push_str(&format!("    script [{}] ({} stmts)\n",
                    script.hat, script.body_length));
            }
            for proc in &target.procedures {
                out.push_str(&format!("    proc {}({}) frame={} ({} stmts)\n",
                    proc.name, proc.param_count, proc.frame_size, proc.body_length));
            }
        }
        out
    }

    /// Return a JSON representation.
    pub fn to_json(&self, project: &Project) -> Result<String, serde_json::Error> {
        let summary = self.explore_project(project);
        serde_json::to_string_pretty(&summary)
    }
}
