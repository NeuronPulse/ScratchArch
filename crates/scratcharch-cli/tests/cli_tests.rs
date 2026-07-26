use std::path::PathBuf;
use std::process::Command;

fn scratcharch_bin() -> PathBuf {
    // CARGO_BIN_EXE_SCRATCHARCH is set by cargo when running integration tests
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_SCRATCHARCH") {
        return PathBuf::from(path);
    }
    // Fallback: CARGO_MANIFEST_DIR is crates/scratcharch-cli
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());
    let mut path = PathBuf::from(&manifest_dir);
    path.pop(); // remove scratcharch-cli/
    path.pop(); // remove crates/
    path.push("target");
    path.push("debug");
    path.push("scratcharch");
    path
}

fn minimal_project_json_str() -> String {
    serde_json::json!({
        "targets": [{
            "isStage": true,
            "name": "Stage",
            "variables": {},
            "lists": {},
            "broadcasts": {},
            "blocks": {}
        }]
    })
    .to_string()
}

fn run_cmd(args: &[&str]) -> (bool, String) {
    let output = Command::new(scratcharch_bin())
        .args(args)
        .output()
        .expect("failed to run scratcharch");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), format!("{}{}", stdout, stderr))
}

#[test]
fn test_cli_inspect_minimal() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_inspect_minimal.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&["inspect", tmp.to_str().unwrap()]);
    assert!(ok, "inspect failed: {}", output);
    assert!(output.contains("Stage"), "output: {}", output);
    assert!(output.contains("scripts: 0"), "output: {}", output);
}

#[test]
fn test_cli_analyze_minimal() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_analyze_minimal.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&["analyze", tmp.to_str().unwrap(), "--format", "json"]);
    assert!(ok, "analyze failed: {}", output);
    assert!(output.contains("project_name"), "output: {}", output);
}

#[test]
fn test_cli_diff_identical() {
    let json = minimal_project_json_str();
    let tmp_a = std::env::temp_dir().join("cli_test_diff_a.json");
    let tmp_b = std::env::temp_dir().join("cli_test_diff_b.json");
    std::fs::write(&tmp_a, &json).unwrap();
    std::fs::write(&tmp_b, &json).unwrap();

    let (ok, output) = run_cmd(&["diff", tmp_a.to_str().unwrap(), tmp_b.to_str().unwrap()]);
    assert!(ok, "diff failed: {}", output);
    assert!(output.contains("No semantic differences"), "output: {}", output);
}

#[test]
fn test_cli_graph_callgraph() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_graph_callgraph.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&["graph", tmp.to_str().unwrap(), "--kind", "callgraph"]);
    assert!(ok, "graph failed: {}", output);
    assert!(output.contains("digraph"), "output: {}", output);
}

#[test]
fn test_cli_decompile_minimal() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_decompile_min.json");
    std::fs::write(&tmp, &json).unwrap();
    let out = std::env::temp_dir().join("cli_test_decompile_out.sair");

    let (ok, output) = run_cmd(&[
        "decompile",
        tmp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(ok, "decompile failed: {}", output);
    assert!(out.exists(), "output file not created: {:?}", out);
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("decompiled from Scratch"), "content: {}", content);
}

#[test]
fn test_cli_inspect_json_minimal() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_inspect_json_min.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&["inspect", tmp.to_str().unwrap(), "--json"]);
    assert!(ok, "inspect --json failed: {}", output);
    assert!(output.contains("project_name"), "output: {}", output);
    assert!(output.contains("targets"), "output: {}", output);
}

#[test]
fn test_cli_inspect_sair() {
    let sair_text = r#"sair 0.1
entry "main"

func @main -> i32 entry "entry" {
  block "entry":
    %0 = const i32 42
    ret i32 %0
}
"#;
    let tmp = std::env::temp_dir().join("cli_test_inspect_sair.sair");
    std::fs::write(&tmp, sair_text).unwrap();

    let (ok, output) = run_cmd(&["inspect", tmp.to_str().unwrap()]);
    assert!(ok, "inspect sair failed: {}", output);
    assert!(output.contains("SAIR Module"), "output: {}", output);
    assert!(output.contains("main"), "output: {}", output);
}

#[test]
fn test_cli_debug_source_map() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_debug_src.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&["debug", tmp.to_str().unwrap()]);
    assert!(ok, "debug failed: {}", output);
    assert!(output.contains("Source map"), "output: {}", output);
}

#[test]
fn test_cli_pipeline_roundtrip() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_pipeline_roundtrip.json");
    let out = std::env::temp_dir().join("cli_test_pipeline_roundtrip_out.json");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&[
        "pipeline",
        tmp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--mode",
        "roundtrip",
    ]);
    assert!(ok, "pipeline roundtrip failed: {}", output);
    assert!(out.exists(), "output file not created: {:?}", out);
}

#[test]
fn test_cli_pipeline_decompile() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_pipeline_decompile.json");
    let out = std::env::temp_dir().join("cli_test_pipeline_decompile_out.sair");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&[
        "pipeline",
        tmp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--mode",
        "decompile",
    ]);
    assert!(ok, "pipeline decompile failed: {}", output);
    assert!(out.exists(), "output file not created: {:?}", out);
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("sair 0.1"), "output: {}", content);
}

#[test]
fn test_cli_pipeline_dump() {
    let json = minimal_project_json_str();
    let tmp = std::env::temp_dir().join("cli_test_pipeline_dump.json");
    let dump_dir = std::env::temp_dir().join("cli_test_pipeline_dump_out");
    std::fs::write(&tmp, &json).unwrap();

    let (ok, output) = run_cmd(&[
        "pipeline",
        tmp.to_str().unwrap(),
        "--dump",
        dump_dir.to_str().unwrap(),
        "--mode",
        "decompile",
    ]);
    assert!(ok, "pipeline dump failed: {}", output);
    // Should have created dump dir with intermediate files
    assert!(dump_dir.join("stage0-input.json").exists(), "dump dir missing stage0");
    assert!(dump_dir.join("stage2-sair.txt").exists(), "dump dir missing stage2");
}

fn write_sb3_project_json() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let sb3_path = dir.path().join("test.sb3");

    // Use scratcharch-sb3 to create a minimal archive
    let stage = scratcharch_scratchgraph::Stage::new("Stage");
    let project = scratcharch_scratchgraph::Project::new().with_stage(stage);
    let writer = scratcharch_sb3::Sb3Writer::new();
    let archive = writer.write(&project);
    let bytes = archive.to_bytes().unwrap();
    std::fs::write(&sb3_path, &bytes).unwrap();
    (dir, sb3_path)
}

#[test]
fn test_cli_sb3_unpack() {
    let (_dir, sb3_path) = write_sb3_project_json();
    let out_dir = std::env::temp_dir().join("cli_test_sb3_unpack_out");
    let _ = std::fs::remove_dir_all(&out_dir);

    let (ok, output) = run_cmd(&[
        "sb3",
        "unpack",
        sb3_path.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(ok, "sb3 unpack failed: {}", output);
    assert!(out_dir.join("project.json").exists(), "project.json not created");
}

#[test]
fn test_cli_sb3_inspect() {
    let (_dir, sb3_path) = write_sb3_project_json();

    let (ok, output) = run_cmd(&["sb3", "inspect", sb3_path.to_str().unwrap()]);
    assert!(ok, "sb3 inspect failed: {}", output);
    assert!(output.contains("Stage"), "output: {}", output);
    assert!(output.contains("Targets"), "output: {}", output);
}

#[test]
fn test_cli_sb3_build() {
    let (_dir, sb3_path) = write_sb3_project_json();
    let out_path = std::env::temp_dir().join("cli_test_sb3_build_out.sb3");

    let (ok, output) = run_cmd(&[
        "sb3",
        "build",
        sb3_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert!(ok, "sb3 build failed: {}", output);
    assert!(out_path.exists(), "output sb3 not created: {:?}", out_path);

    // Verify the output is a valid sb3 archive
    let bytes = std::fs::read(&out_path).unwrap();
    let archive = scratcharch_sb3::Sb3Archive::from_bytes(&bytes).unwrap();
    assert_eq!(archive.project.targets.len(), 1);
}
