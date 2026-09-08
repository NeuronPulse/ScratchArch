//! Differential corpus harness.
//!
//! The corpus under `tests/corpus/scratch/` is a committed set of loadable
//! `project.json` fixtures — one per representative native-Scratch program —
//! plus a machine-readable `manifest.json` catalog. It exists so future
//! automatic regression can iterate over the same representative projects the
//! framework is validated against, without re-deriving them from Rust.
//!
//! This harness keeps the corpus honest in two directions:
//!
//! 1. **The fixture file is the source of truth.** Each member is loaded from
//!    disk through the same entry the CLI uses for `.json` input
//!    (`parse_project_json`), then must pass graph validation and the full
//!    `verify_project` gate (graph + SB3 roundtrip + semantic preservation +
//!    per-pass transform preservation).
//! 2. **The file and the builder must agree.** Each committed fixture is also
//!    compared, after normalization, with the member as authored in
//!    `members.rs`. If the exporter format, the parser, or the canonical IR
//!    ever stops round-tripping a representative program's meaning, or a
//!    member is edited without regenerating its fixture, this harness reports
//!    the drift instead of silently blessing it.
//!
//! Members are committed as data on purpose: this is a *corpus*, so a member
//! is the `project.json` a future harness or tool loads, not the Rust that
//! produced it. Regenerating fixtures is a deliberate, env-gated act (see
//! [`regenerate_corpus_fixtures`]).

use std::fs;
use std::path::{Path, PathBuf};

use scratcharch_scratchgraph::{parse_project_json, Project, ScratchExporter, SemanticNormalizer};
use scratcharch_validation::{validate_graph, verify_project};

#[path = "corpus/members.rs"]
mod members;

/// Absolute directory holding the committed corpus fixtures.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("corpus").join("scratch")
}

/// Semantic equality: normalize both projects and compare.
fn semantically_equal(a: &Project, b: &Project) -> bool {
    let n = SemanticNormalizer::new();
    n.normalize(a) == n.normalize(b)
}

/// Load one manifest entry's fixture file through the real `.json` parser.
fn load_fixture(dir: &Path, file: &str) -> Project {
    let path = dir.join(file);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("corpus fixture {} unreadable: {e}", path.display()));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
    parse_project_json(&value)
        .unwrap_or_else(|e| panic!("fixture {} failed to parse: {e}", path.display()))
}

#[test]
fn committed_fixtures_are_cataloged_and_complete() {
    let dir = corpus_dir();
    let manifest_path = dir.join("manifest.json");
    assert!(manifest_path.exists(), "missing corpus manifest at {}", manifest_path.display());

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("read corpus manifest"),
    )
    .expect("corpus manifest is valid JSON");
    let listed = manifest
        .get("members")
        .and_then(|m| m.as_array())
        .expect("manifest has a members array");

    // Every cataloged member has a committed fixture file.
    for entry in listed {
        let file = entry.get("file").and_then(|f| f.as_str()).expect("member has a file");
        let id = entry.get("id").and_then(|f| f.as_str()).unwrap_or(file);
        assert!(
            dir.join(file).exists(),
            "member '{id}' cataloged in manifest but fixture {file} is missing"
        );
    }

    // Every builder member is cataloged (no orphan builders silently dropped).
    let cataloged_ids: Vec<&str> = listed
        .iter()
        .filter_map(|e| e.get("id").and_then(|f| f.as_str()))
        .collect();
    for m in members::members() {
        assert!(
            cataloged_ids.contains(&m.id),
            "builder member '{}' has no manifest entry",
            m.id
        );
    }

    // Representative coverage across every category the framework is built on.
    for required in ["basic", "events", "procedures", "recursion", "lists", "memory"] {
        assert!(
            listed
                .iter()
                .any(|e| e.get("category").and_then(|c| c.as_str()) == Some(required)),
            "corpus has no {required} member"
        );
    }
}

#[test]
fn every_fixture_passes_graph_validation_and_verify() {
    let dir = corpus_dir();
    for m in members::members() {
        let fixture = load_fixture(&dir, &format!("{}.json", m.id));

        validate_graph(&fixture)
            .unwrap_or_else(|issues| panic!("[{}] graph validation failed: {issues:?}", m.id));

        let report = verify_project(&fixture);
        assert!(
            report.passed(),
            "[{}] verify failed:\n{}",
            m.id,
            report.to_text()
        );
    }
}

#[test]
fn committed_fixture_matches_its_builder_semantics() {
    let dir = corpus_dir();
    for m in members::members() {
        let fixture = load_fixture(&dir, &format!("{}.json", m.id));
        assert!(
            semantically_equal(&fixture, &m.project),
            "[{}] committed fixture no longer matches the member builder: the file and the \
             canonical IR disagree. If the member changed, regenerate the corpus; if a parser \
             or exporter change altered this program's meaning, fix the regression.",
            m.id
        );
    }
}

/// Regenerate every committed fixture and the manifest from the member
/// builders in `members.rs`.
///
/// Deliberate and env-gated: run with
/// `SCRATCHARCH_REGEN_CORPUS=1 cargo test -p scratcharch-validation \
///   --test corpus regenerate_corpus_fixtures -- --ignored --nocapture`.
#[test]
#[ignore = "rewrites committed corpus fixtures; opt in explicitly"]
fn regenerate_corpus_fixtures() {
    if std::env::var("SCRATCHARCH_REGEN_CORPUS").as_deref() != Ok("1") {
        eprintln!("skip: set SCRATCHARCH_REGEN_CORPUS=1 to regenerate the corpus");
        return;
    }
    let dir = corpus_dir();
    fs::create_dir_all(&dir).expect("create corpus scratch dir");

    let mut manifest_members = Vec::new();
    for m in members::members() {
        // JsonExporter::Error is the never-occurring Infallible type.
        let value = scratcharch_scratchgraph::JsonExporter::new()
            .export(&m.project)
            .expect("export member to project.json");
        let json = serde_json::to_string_pretty(&value).expect("serialize member json");
        let file = format!("{}.json", m.id);
        fs::write(dir.join(&file), format!("{json}\n"))
            .unwrap_or_else(|e| panic!("write {file}: {e}"));

        manifest_members.push(serde_json::json!({
            "id": m.id,
            "category": m.category,
            "description": m.description,
            "file": file,
        }));
    }

    let manifest = serde_json::json!({
        "schema": "scratcharch-corpus",
        "members": manifest_members,
    });
    fs::write(
        dir.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).expect("serialize manifest")),
    )
    .expect("write manifest");

    eprintln!("regenerated {} corpus fixtures under {}", members::members().len(), dir.display());
}
