use scratcharch_scratchgraph::ir::{Costume, Project, Sound, Sprite, Stage};
use serde_json::{json, Value};

use crate::archive::Sb3Archive;
use crate::asset::AssetManager;
use crate::project::{CostumeItem, Meta, Sb3Project, SoundItem, Target};

/// Converts a ScratchGraph Project into an Sb3Archive.
#[derive(Debug, Clone, Default)]
pub struct Sb3Writer;

impl Sb3Writer {
    pub fn new() -> Self {
        Self
    }

    /// Convert a ScratchGraph Project to an Sb3Archive.
    ///
    /// The returned archive has an empty AssetManager; assets must be
    /// populated separately via `archive.assets.add()`.
    pub fn write(&self, project: &Project) -> Sb3Archive {
        let mut targets = Vec::new();

        targets.push(target_from_stage(&project.stage));

        for sprite in &project.sprites {
            targets.push(target_from_sprite(sprite));
        }

        let sb3_project = Sb3Project {
            targets,
            monitors: Vec::new(),
            extensions: Vec::new(),
            meta: Meta {
                semver: "3.0.0".to_string(),
                vm: "0.2.0".to_string(),
                agent: "ScratchArch".to_string(),
            },
        };

        Sb3Archive {
            project: sb3_project,
            assets: AssetManager::new(),
        }
    }
}

fn target_from_stage(stage: &Stage) -> Target {
    let mut target = base_target(&stage.name, true);

    let mut variables = serde_json::Map::new();
    for v in &stage.variables {
        variables.insert(v.id.clone(), Value::Array(vec![json!(v.name), json!("")]));
    }
    target.variables = Value::Object(variables);

    let mut lists = serde_json::Map::new();
    for l in &stage.lists {
        lists.insert(l.id.clone(), Value::Array(vec![json!(l.name), json!([])]));
    }
    target.lists = Value::Object(lists);

    let mut broadcasts = serde_json::Map::new();
    for b in &stage.broadcasts {
        broadcasts.insert(b.id.clone(), json!(b.name));
    }
    target.broadcasts = Value::Object(broadcasts);

    target.costumes = costumes_to_items(&stage.costumes);
    target.sounds = sounds_to_items(&stage.sounds);

    target
}

fn target_from_sprite(sprite: &Sprite) -> Target {
    let mut target = base_target(&sprite.name, false);

    let mut variables = serde_json::Map::new();
    for v in &sprite.variables {
        variables.insert(v.id.clone(), Value::Array(vec![json!(v.name), json!("")]));
    }
    target.variables = Value::Object(variables);

    let mut lists = serde_json::Map::new();
    for l in &sprite.lists {
        lists.insert(l.id.clone(), Value::Array(vec![json!(l.name), json!([])]));
    }
    target.lists = Value::Object(lists);

    let mut broadcasts = serde_json::Map::new();
    for b in &sprite.broadcasts {
        broadcasts.insert(b.id.clone(), json!(b.name));
    }
    target.broadcasts = Value::Object(broadcasts);

    target.costumes = costumes_to_items(&sprite.costumes);
    target.sounds = sounds_to_items(&sprite.sounds);

    target
}

fn base_target(name: &str, is_stage: bool) -> Target {
    Target {
        name: name.to_string(),
        is_stage,
        x: 0.0,
        y: 0.0,
        size: 100.0,
        direction: 90.0,
        draggable: false,
        rotation_style: "all around".to_string(),
        visible: true,
        costume_index: 0,
        sound_index: 0,
        volume: 100.0,
        tempo: 60.0,
        video_transparency: 50.0,
        video_state: "off".to_string(),
        variables: serde_json::Value::Object(serde_json::Map::new()),
        lists: serde_json::Value::Object(serde_json::Map::new()),
        broadcasts: serde_json::Value::Object(serde_json::Map::new()),
        blocks: serde_json::Value::Object(serde_json::Map::new()),
        costumes: Vec::new(),
        sounds: Vec::new(),
    }
}

fn costumes_to_items(costumes: &[Costume]) -> Vec<CostumeItem> {
    costumes
        .iter()
        .map(|c| CostumeItem {
            name: c.name.clone(),
            asset_id: c.asset_id.clone(),
            md5ext: c.asset_id.clone(),
            data_format: c
                .asset_id
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_string())
                .unwrap_or_default(),
            bitmap: c.bitmap,
            rotation_center_x: c.rotation_center_x,
            rotation_center_y: c.rotation_center_y,
        })
        .collect()
}

fn sounds_to_items(sounds: &[Sound]) -> Vec<SoundItem> {
    sounds
        .iter()
        .map(|s| SoundItem {
            name: s.name.clone(),
            asset_id: s.asset_id.clone(),
            md5ext: s.asset_id.clone(),
            data_format: s
                .asset_id
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_string())
                .unwrap_or_default(),
            rate: s.rate,
            sample_count: s.sample_count,
            format: s.format.clone(),
        })
        .collect()
}
