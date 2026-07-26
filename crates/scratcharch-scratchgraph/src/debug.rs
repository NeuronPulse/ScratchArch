use serde::{Deserialize, Serialize};

/// Source location mapping between SAIR, ScratchGraph, and project.json.
///
/// Each `SourceLocation` records where a ScratchGraph IR node came from in
/// the original Scratch project, enabling forward queries such as
/// "which Scratch block does this SAIR instruction correspond to?"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub project_id: Option<String>,
    pub sprite_name: String,
    pub script_id: Option<String>,
    pub block_id: Option<String>,
    pub opcode: Option<String>,
}

/// Per-target mapping from Scratch block IDs to `SourceLocation` entries.
///
/// For each target (stage or sprite), maps every parsed block ID to its
/// source location.  This is a side-table: the `Project` IR itself does not
/// store block IDs (they are JSON serialization details), but analyses and
/// debuggers can look up the mapping to answer queries.
pub type SourceMap = Vec<TargetSourceMap>;

/// Source map for a single target (stage or sprite).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSourceMap {
    pub target_name: String,
    pub blocks: Vec<BlockSourceEntry>,
}

/// A single source-location record binding a JSON block ID to its source info.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSourceEntry {
    pub block_id: String,
    pub opcode: String,
    pub is_top_level: bool,
    pub parent: Option<String>,
}

impl SourceLocation {
    pub fn new(sprite_name: impl Into<String>) -> Self {
        SourceLocation {
            project_id: None,
            sprite_name: sprite_name.into(),
            script_id: None,
            block_id: None,
            opcode: None,
        }
    }

    pub fn with_project_id(mut self, id: impl Into<String>) -> Self {
        self.project_id = Some(id.into());
        self
    }

    pub fn with_script_id(mut self, id: impl Into<String>) -> Self {
        self.script_id = Some(id.into());
        self
    }

    pub fn with_block_id(mut self, id: impl Into<String>) -> Self {
        self.block_id = Some(id.into());
        self
    }

    pub fn with_opcode(mut self, opcode: impl Into<String>) -> Self {
        self.opcode = Some(opcode.into());
        self
    }
}

impl TargetSourceMap {
    pub fn new(target_name: impl Into<String>) -> Self {
        TargetSourceMap {
            target_name: target_name.into(),
            blocks: Vec::new(),
        }
    }
}
