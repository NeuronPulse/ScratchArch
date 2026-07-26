use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Top-level Scratch 3 project.json structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sb3Project {
    pub targets: Vec<Target>,
    #[serde(default)]
    pub monitors: Vec<Value>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Meta {
    #[serde(default)]
    pub semver: String,
    #[serde(default)]
    pub vm: String,
    #[serde(default)]
    pub agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub name: String,
    #[serde(default)]
    pub is_stage: bool,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub size: f64,
    #[serde(default)]
    pub direction: f64,
    #[serde(default)]
    pub draggable: bool,
    #[serde(default)]
    pub rotation_style: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub costume_index: u32,
    #[serde(default)]
    pub sound_index: u32,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub tempo: f64,
    #[serde(default)]
    pub video_transparency: f64,
    #[serde(default)]
    pub video_state: String,
    #[serde(default)]
    pub variables: Value,
    #[serde(default)]
    pub lists: Value,
    #[serde(default)]
    pub broadcasts: Value,
    #[serde(default)]
    pub blocks: Value,
    #[serde(default)]
    pub costumes: Vec<CostumeItem>,
    #[serde(default)]
    pub sounds: Vec<SoundItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostumeItem {
    pub name: String,
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub md5ext: String,
    #[serde(default)]
    pub data_format: String,
    #[serde(default)]
    pub bitmap: bool,
    #[serde(default)]
    pub rotation_center_x: f64,
    #[serde(default)]
    pub rotation_center_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundItem {
    pub name: String,
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub md5ext: String,
    #[serde(default)]
    pub data_format: String,
    #[serde(default)]
    pub rate: u32,
    #[serde(default)]
    pub sample_count: u32,
    #[serde(default)]
    pub format: String,
}

impl Sb3Project {
    pub fn from_json(value: &Value) -> Result<Self, String> {
        serde_json::from_value(value.clone()).map_err(|e| format!("SB3 project parse error: {e}"))
    }

    pub fn to_json(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| format!("SB3 project serialize error: {e}"))
    }
}
