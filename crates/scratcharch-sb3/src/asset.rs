use md5::{Digest, Md5};
use std::collections::HashMap;

/// Binary asset data extracted from an sb3 archive.
#[derive(Debug, Clone)]
pub struct AssetData {
    /// md5ext filename (e.g. "abc123.svg").
    pub md5ext: String,
    /// Raw file bytes.
    pub data: Vec<u8>,
}

/// Manages assets extracted from an sb3 archive, keyed by md5ext.
#[derive(Debug, Clone, Default)]
pub struct AssetManager {
    assets: HashMap<String, AssetData>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an asset by its md5ext and raw data.
    pub fn add(&mut self, md5ext: String, data: Vec<u8>) {
        let ad = AssetData {
            md5ext: md5ext.clone(),
            data,
        };
        self.assets.insert(md5ext, ad);
    }

    /// Retrieve an asset by md5ext.
    pub fn get(&self, md5ext: &str) -> Option<&AssetData> {
        self.assets.get(md5ext)
    }

    /// Check if an asset exists.
    pub fn contains(&self, md5ext: &str) -> bool {
        self.assets.contains_key(md5ext)
    }

    /// Number of stored assets.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Whether there are no stored assets.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Iterate over all assets.
    pub fn iter(&self) -> impl Iterator<Item = &AssetData> {
        self.assets.values()
    }

    /// Consume and return all assets.
    pub fn into_assets(self) -> Vec<AssetData> {
        self.assets.into_values().collect()
    }

    /// Verify that an asset's MD5 hash matches its md5ext name.
    pub fn verify_asset(asset: &AssetData) -> bool {
        let computed = compute_md5(&asset.data);
        let expected = asset.md5ext.rsplit_once('.').map(|(h, _)| h).unwrap_or(&asset.md5ext);
        computed == expected
    }

    /// Compute the md5ext for raw data with a given extension.
    pub fn compute_md5ext(data: &[u8], extension: &str) -> String {
        let hash = compute_md5(data);
        format!("{hash}.{extension}")
    }
}

fn compute_md5(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{:x}", result)
}
