//! Corpus manifest (`tests/corpus/llvm/manifest.json`).
//!
//! Each fixture records what it is and what the toolchain is *expected* to do
//! with it — the manifest doubles as the regression oracle (Part 10): the
//! runner's gate is "actual outcome == recorded expectation", so every new
//! milestone either keeps old fixtures green or fails loudly.
//!
//! Paths (`source`, `llvm_ir`) are repo-root-relative so the manifest can
//! reference committed real-clang fixtures wherever they live (existing green
//! fixtures stay in `tests/c_programs/`; feature-gap fixtures live under
//! `tests/corpus/llvm/`).
//!
//! **Expectation semantics.** `expected_stage` names the first stage the
//! fixture is *not* fully expected to work at; every report stage before it
//! must pass. `expected_status` then says what happens there:
//!
//! * `"success"` — the fixture works through every stage, so
//!   `expected_stage` is `scratch`.
//! * `"unsupported"` — the boundary stage is a *known capability gap*: the
//!   fixture runs correctly through the earlier stages and the boundary stage
//!   rejects it with a stable diagnostic (pinned by the optional `vm` /
//!   `scratch` `"rejected"` fields).
//!
//! ```json
//! {
//!   "version": "0.1",
//!   "fixtures": [
//!     {
//!       "name": "add",
//!       "source": "tests/c_programs/add.c",
//!       "llvm_ir": "tests/c_programs/add.ll",
//!       "feature_tags": ["integer"],
//!       "expected_result": 42,
//!       "expected_stage": "scratch",
//!       "expected_status": "success",
//!       "vm": { "exact": 42 },
//!       "scratch": { "constructs": true }
//!     }
//!   ]
//! }
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::status::{FEATURES, Stage};

/// Manifest JSON with no semantic checks (validated by [`Manifest::validate`]).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestFile {
    /// Corpus version, e.g. "0.1". Recorded in baselines.
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub fixtures: Vec<FixtureEntry>,
}

fn default_version() -> String {
    "0.0".to_string()
}

/// Expected behavior of the Scratch backend (SAIR → ScratchGraph lowering).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum BackendExpect {
    /// The VM returns this exact value.
    Exact { exact: u64 },
    /// The Scratch backend constructs a project.
    Constructs { constructs: bool },
    /// The backend rejects the module; `rejected` is a substring of the
    /// diagnostic it must produce.
    Rejected { rejected: String },
}

impl BackendExpect {
    /// `true` when the backend is expected to succeed on this fixture.
    pub fn expects_pass(&self) -> bool {
        match self {
            BackendExpect::Exact { .. } | BackendExpect::Constructs { constructs: true } => true,
            BackendExpect::Constructs { constructs: false } => false,
            BackendExpect::Rejected { .. } => false,
        }
    }
}

/// One manifest fixture record.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FixtureEntry {
    /// Unique fixture name (matches the committed `.c`/`.ll` basename).
    pub name: String,
    /// Repo-root-relative path to the C source (absent for pure-`.ll` fixtures).
    #[serde(default)]
    pub source: Option<String>,
    /// Repo-root-relative path to the LLVM IR. Real-clang fixtures carry both.
    #[serde(default)]
    pub llvm_ir: Option<String>,
    /// Canonical feature tags (subset of [`FEATURES`]).
    #[serde(default)]
    pub feature_tags: Vec<String>,
    /// The exit value `main` returns (the result channel), when numeric.
    #[serde(default)]
    pub expected_result: Option<u64>,
    /// Last stage the fixture is expected to fully pass.
    #[serde(default)]
    pub expected_stage: Option<String>,
    /// `"success"` (full green) or `"unsupported"` (known capability gap right
    /// after `expected_stage`).
    #[serde(default)]
    pub expected_status: Option<String>,
    /// Optional VM expectation (`exact`/`rejected`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm: Option<BackendExpect>,
    /// Optional Scratch expectation (`constructs`/`rejected`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scratch: Option<BackendExpect>,
}

/// Loaded, validated manifest plus its resolution root.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub version: String,
    pub fixtures: Vec<FixtureEntry>,
    /// Directory all repo-root-relative paths resolve against.
    pub root: PathBuf,
}

impl Manifest {
    /// Parse manifest text (repo-root-relative paths resolve from `root`).
    pub fn parse(text: &str, root: impl Into<PathBuf>) -> Result<Self, String> {
        let mut file: ManifestFile =
            serde_json::from_str(text).map_err(|e| format!("manifest JSON: {e}"))?;
        // Canonicalize feature tags onto the canonical taxonomy before storing.
        for fx in file.fixtures.iter_mut() {
            for tag in fx.feature_tags.iter_mut() {
                *tag = Self::canonical_tag(tag).unwrap_or_else(|| tag.clone());
            }
        }
        let man = Manifest {
            version: file.version,
            fixtures: file.fixtures,
            root: root.into(),
        };
        man.validate()?;
        Ok(man)
    }

    /// Load and validate a manifest from disk, resolving paths from `root`.
    pub fn load(path: &Path, root: impl Into<PathBuf>) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text, root)
    }

    /// Resolve a repo-root-relative path recorded in the manifest.
    pub fn resolve(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Canonicalize a feature tag, mapping aliases onto the canonical set.
    fn canonical_tag(tag: &str) -> Option<String> {
        // Accept "globals" (as written in some taxonomy lists) as "global".
        let canonical = match tag {
            "globals" => "global",
            t => t,
        };
        if FEATURES.contains(&canonical) {
            Some(canonical.to_string())
        } else {
            None
        }
    }

    /// Semantic validation: names unique, paths present, tags canonical,
    /// `success` implies `expected_stage == scratch`.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for (i, fx) in self.fixtures.iter().enumerate() {
            if fx.name.trim().is_empty() {
                return Err(format!("fixture #{i}: empty name"));
            }
            if !seen.insert(fx.name.clone()) {
                return Err(format!("fixture #{i}: duplicate name {:?}", fx.name));
            }
            if fx.llvm_ir.is_none() && fx.source.is_none() {
                return Err(format!(
                    "fixture {:?}: needs at least one of llvm_ir / source",
                    fx.name
                ));
            }
            for tag in &fx.feature_tags {
                if Self::canonical_tag(tag).is_none() {
                    return Err(format!(
                        "fixture {:?}: unknown feature tag {:?} (canonical: {})",
                        fx.name,
                        tag,
                        FEATURES.join(", ")
                    ));
                }
            }
            let stage = fx.expected_stage.as_deref().and_then(Stage::from_str);
            let stage = stage.ok_or_else(|| {
                format!(
                    "fixture {:?}: invalid expected_stage {:?}",
                    fx.name, fx.expected_stage
                )
            })?;
            match fx.expected_status.as_deref() {
                Some("success") => {
                    if stage != Stage::Scratch {
                        return Err(format!(
                            "fixture {:?}: expected_status=success requires expected_stage=scratch",
                            fx.name
                        ));
                    }
                }
                Some("unsupported") => {}
                other => {
                    return Err(format!(
                        "fixture {:?}: invalid expected_status {:?}",
                        fx.name, other
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
        "version": "0.1",
        "fixtures": [
            {
                "name": "add",
                "source": "tests/c_programs/add.c",
                "llvm_ir": "tests/c_programs/add.ll",
                "feature_tags": ["integer", "globals"],
                "expected_result": 42,
                "expected_stage": "scratch",
                "expected_status": "success",
                "vm": { "exact": 42 },
                "scratch": { "constructs": true }
            }
        ]
    }"#;

    #[test]
    fn parses_and_canonicalizes_tags() {
        let m = Manifest::parse(MINIMAL, "/repo").expect("parse");
        assert_eq!(m.version, "0.1");
        assert_eq!(m.fixtures.len(), 1);
        let fx = &m.fixtures[0];
        assert_eq!(fx.name, "add");
        assert_eq!(fx.expected_result, Some(42));
        assert_eq!(fx.vm, Some(BackendExpect::Exact { exact: 42 }));
        assert_eq!(
            fx.scratch,
            Some(BackendExpect::Constructs { constructs: true })
        );
        // Paths resolve against the root.
        assert_eq!(m.resolve(fx.llvm_ir.as_ref().unwrap()), PathBuf::from("/repo/tests/c_programs/add.ll"));
    }

    #[test]
    fn rejects_unknown_feature_tag() {
        let bad = MINIMAL.replace("integer\", \"globals", "integer\", \"bogus");
        let err = Manifest::parse(&bad, "/repo").expect_err("must reject unknown tag");
        assert!(err.contains("unknown feature tag"), "got: {err}");
    }

    #[test]
    fn rejects_duplicate_names() {
        let dup = r#"{
            "version": "0.1",
            "fixtures": [
                {"name":"a","llvm_ir":"a.ll","feature_tags":["integer"],
                 "expected_stage":"scratch","expected_status":"success"},
                {"name":"a","llvm_ir":"b.ll","feature_tags":["integer"],
                 "expected_stage":"scratch","expected_status":"success"}
            ]
        }"#;
        let err = Manifest::parse(dup, "/repo").expect_err("must reject duplicate");
        assert!(err.contains("duplicate name"), "got: {err}");
    }

    #[test]
    fn rejects_success_without_scratch_stage() {
        let bad = MINIMAL.replace("\"expected_stage\": \"scratch\"", "\"expected_stage\": \"vm\"");
        let err = Manifest::parse(&bad, "/repo").expect_err("success requires scratch stage");
        assert!(err.contains("requires expected_stage=scratch"), "got: {err}");
    }

    #[test]
    fn missing_both_paths_is_invalid() {
        let bad = r#"{
            "version": "0.1",
            "fixtures": [
                {"name":"orphan","feature_tags":["integer"],
                 "expected_stage":"scratch","expected_status":"success"}
            ]
        }"#;
        let err = Manifest::parse(bad, "/repo").expect_err("must reject orphan fixture");
        assert!(err.contains("needs at least one"), "got: {err}");
    }

    #[test]
    fn vm_rejected_parses() {
        let s = r#"{
            "version": "0.1",
            "fixtures": [{
                "name":"memintrin","llvm_ir":"m.ll","feature_tags":["memory","intrinsic"],
                "expected_result":1,"expected_stage":"interpreter","expected_status":"unsupported",
                "vm":{"rejected":"undefined function: llvm.memcpy.p0.p0.i64"},
                "scratch":{"constructs":true}
            }]
        }"#;
        let m = Manifest::parse(s, "/repo").expect("parse");
        assert_eq!(m.fixtures[0].expected_status.as_deref(), Some("unsupported"));
        assert_eq!(
            m.fixtures[0].vm,
            Some(BackendExpect::Rejected {
                rejected: "undefined function: llvm.memcpy.p0.p0.i64".to_string()
            })
        );
    }
}
