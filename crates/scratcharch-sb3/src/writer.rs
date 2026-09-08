use scratcharch_scratchgraph::{JsonExporter, ScratchExporter};
use serde_json::Value;

use crate::archive::Sb3Archive;
use crate::asset::AssetManager;
use crate::project::Sb3Project;

/// Converts a ScratchGraph Project into an Sb3Archive.
#[derive(Debug, Clone, Default)]
pub struct Sb3Writer;

impl Sb3Writer {
    pub fn new() -> Self {
        Self
    }

    /// Convert a ScratchGraph Project to an Sb3Archive with empty assets.
    ///
    /// For preserving assets from an existing archive, use
    /// [`Self::write_with_assets`].
    pub fn write(&self, project: &scratcharch_scratchgraph::ir::Project) -> Sb3Archive {
        self.write_with_assets(project, AssetManager::new())
            .expect("JSON produced by JsonExporter deserializes into Sb3Project")
    }

    /// Convert a ScratchGraph Project to an Sb3Archive while preserving assets.
    ///
    /// This uses `JsonExporter` to produce a full Scratch 3 `project.json`
    /// (including scripts, procedures, variables, lists, and broadcasts) and
    /// then deserializes it into an `Sb3Project`. ScratchGraph semantics are
    /// not modified here; this is purely serialization.
    pub fn write_with_assets(
        &self,
        project: &scratcharch_scratchgraph::ir::Project,
        assets: AssetManager,
    ) -> Result<Sb3Archive, String> {
        let json_value = JsonExporter::new()
            .export(project)
            .map_err(|_| "Sb3Writer: JSON export failed")?;
        let sb3_project = Sb3Project::from_json(&json_value)?;

        Ok(Sb3Archive {
            project: sb3_project,
            assets,
        })
    }

    /// Convert an already-serialized `project.json` value into an archive.
    ///
    /// This is useful when the caller has produced JSON through another path
    /// and only needs sb3 packaging.
    pub fn write_from_json(
        &self,
        value: &Value,
        assets: AssetManager,
    ) -> Result<Sb3Archive, String> {
        let sb3_project = Sb3Project::from_json(value)?;
        Ok(Sb3Archive {
            project: sb3_project,
            assets,
        })
    }
}
