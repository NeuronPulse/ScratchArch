use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::asset::AssetManager;
use crate::project::Sb3Project;

/// Represents a parsed .sb3 archive with project.json and assets.
#[derive(Debug, Clone)]
pub struct Sb3Archive {
    pub project: Sb3Project,
    pub assets: AssetManager,
}

impl Sb3Archive {
    /// Open and parse an sb3 file from a byte buffer.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let cursor = Cursor::new(data);
        let mut archive =
            ZipArchive::new(cursor).map_err(|e| format!("failed to open sb3 archive: {e}"))?;

        let mut project_json_raw: Option<Vec<u8>> = None;
        let mut asset_map: HashMap<String, Vec<u8>> = HashMap::new();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("failed to read archive entry {i}: {e}"))?;
            let name = file.name().to_string();

            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("failed to read file {name}: {e}"))?;

            if name == "project.json" {
                project_json_raw = Some(contents);
            } else {
                asset_map.insert(name, contents);
            }
        }

        let raw = project_json_raw.ok_or("sb3 archive missing project.json")?;
        let project_value: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| format!("invalid project.json: {e}"))?;
        let project = Sb3Project::from_json(&project_value)?;

        let mut assets = AssetManager::new();
        for (md5ext, data) in asset_map {
            assets.add(md5ext, data);
        }

        Ok(Sb3Archive { project, assets })
    }

    /// Read an sb3 file from disk.
    pub fn from_path(path: &str) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|e| format!("failed to read sb3 file {path}: {e}"))?;
        Self::from_bytes(&data)
    }

    /// Write the archive to a byte buffer (zip format).
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);

        // Write project.json
        let json_value = self
            .project
            .to_json()
            .map_err(|e| format!("failed to serialize project: {e}"))?;
        let json_bytes = serde_json::to_vec_pretty(&json_value)
            .map_err(|e| format!("failed to encode project.json: {e}"))?;
        zip.start_file("project.json", SimpleFileOptions::default())
            .map_err(|e| format!("failed to create project.json in archive: {e}"))?;
        zip.write_all(&json_bytes)
            .map_err(|e| format!("failed to write project.json: {e}"))?;

        // Write asset files
        for asset in self.assets.iter() {
            zip.start_file(&asset.md5ext, SimpleFileOptions::default())
                .map_err(|e| format!("failed to create asset {} in archive: {e}", asset.md5ext))?;
            zip.write_all(&asset.data)
                .map_err(|e| format!("failed to write asset {}: {e}", asset.md5ext))?;
        }

        let result = zip
            .finish()
            .map_err(|e| format!("failed to finalize sb3 archive: {e}"))?;
        Ok(result.into_inner())
    }

    /// Write the archive to disk.
    pub fn to_path(&self, path: &str) -> Result<(), String> {
        let data = self.to_bytes()?;
        std::fs::write(path, &data)
            .map_err(|e| format!("failed to write sb3 file {path}: {e}"))
    }
}
