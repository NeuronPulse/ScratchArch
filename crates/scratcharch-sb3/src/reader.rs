use scratcharch_scratchgraph::ir::{
    Broadcast, Costume, Project, Sound, Sprite, Stage, VariableScope,
};

use crate::archive::Sb3Archive;

/// Converts an Sb3Archive into a ScratchGraph Project.
#[derive(Debug, Clone, Default)]
pub struct Sb3Reader;

impl Sb3Reader {
    pub fn new() -> Self {
        Self
    }

    /// Convert the archive to a ScratchGraph Project.
    ///
    /// Variables and lists are assigned global scope (the serde types
    /// in Sb3Project do not carry scope information; actual scope
    /// is determined by which target owns them).
    pub fn read(&self, archive: &Sb3Archive) -> Result<Project, String> {
        let mut stage = None;
        let mut sprites = Vec::new();

        for target in &archive.project.targets {
            let costumes: Vec<Costume> = target
                .costumes
                .iter()
                .map(|c| Costume {
                    name: c.name.clone(),
                    asset_id: c.md5ext.clone(),
                    bitmap: c.bitmap,
                    rotation_center_x: c.rotation_center_x,
                    rotation_center_y: c.rotation_center_y,
                })
                .collect();

            let sounds: Vec<Sound> = target
                .sounds
                .iter()
                .map(|s| Sound {
                    name: s.name.clone(),
                    asset_id: s.md5ext.clone(),
                    rate: s.rate,
                    sample_count: s.sample_count,
                    format: s.format.clone(),
                })
                .collect();

            let variables = parse_variable_map(&target.variables);
            let lists = parse_list_map(&target.lists);
            let broadcasts = parse_broadcast_map(&target.broadcasts);

            // Use the existing public parser for blocks -> scripts/procedures.
            // Reconstruct a full project.json value to use `parse_project_json`.
            let full_json = serde_json::json!({
                "targets": [{
                    "isStage": target.is_stage,
                    "name": target.name,
                    "variables": target.variables,
                    "lists": target.lists,
                    "broadcasts": target.broadcasts,
                    "blocks": target.blocks,
                }]
            });
            let parsed = scratcharch_scratchgraph::parse_project_json(&full_json)
                .map_err(|e| format!("failed to parse blocks: {e}"))?;

            let name = target.name.clone();
            if target.is_stage {
                stage = Some(Stage {
                    name,
                    variables,
                    lists,
                    broadcasts,
                    costumes,
                    sounds,
                    scripts: parsed.stage.scripts,
                    procedures: parsed.stage.procedures,
                });
            } else {
                let parsed_sprite = parsed.sprites.into_iter().next().unwrap_or_else(|| Sprite {
                    name: name.clone(),
                    ..Default::default()
                });
                sprites.push(Sprite {
                    name,
                    variables,
                    lists,
                    broadcasts,
                    costumes,
                    sounds,
                    scripts: parsed_sprite.scripts,
                    procedures: parsed_sprite.procedures,
                });
            }
        }

        let stage = stage.unwrap_or_else(|| Stage {
            name: "Stage".to_string(),
            ..Default::default()
        });

        Ok(Project { stage, sprites })
    }
}

fn parse_variable_map(value: &serde_json::Value) -> Vec<scratcharch_scratchgraph::ir::Variable> {
    let mut result = Vec::new();
    if let Some(obj) = value.as_object() {
        for (id, val) in obj {
            let name = val
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            result.push(scratcharch_scratchgraph::ir::Variable {
                id: id.clone(),
                name,
                scope: VariableScope::Global,
            });
        }
    }
    result
}

fn parse_list_map(value: &serde_json::Value) -> Vec<scratcharch_scratchgraph::ir::List> {
    let mut result = Vec::new();
    if let Some(obj) = value.as_object() {
        for (id, val) in obj {
            let name = val
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            result.push(scratcharch_scratchgraph::ir::List {
                id: id.clone(),
                name,
                scope: scratcharch_scratchgraph::ir::ListScope::Global,
            });
        }
    }
    result
}

fn parse_broadcast_map(
    value: &serde_json::Value,
) -> Vec<scratcharch_scratchgraph::ir::Broadcast> {
    let mut result = Vec::new();
    if let Some(obj) = value.as_object() {
        for (id, val) in obj {
            let name = val.as_str().unwrap_or("").to_string();
            result.push(Broadcast {
                id: id.clone(),
                name,
            });
        }
    }
    result
}
