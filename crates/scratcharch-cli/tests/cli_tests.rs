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

fn make_optimizable_project() -> scratcharch_scratchgraph::ir::Project {
    use scratcharch_scratchgraph::ir::*;
    let mut stage = Stage::new("Stage");
    stage.add_variable(Variable::new("x_id", "x"));
    stage.add_variable(Variable::new("unused_id", "unused"));

    // Green-flag script: foldable expression and a call to my_proc.
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "x".to_string(),
                value: Expr::Operator {
                    opcode: "operator_add".to_string(),
                    args: vec![
                        Expr::Literal(Value::Number(10.0)),
                        Expr::Literal(Value::Number(20.0)),
                    ],
                },
            },
            Stmt::Call {
                proc: "my_proc".to_string(),
                args: vec![],
            },
        ],
    ));

    // Unreachable broadcast script (should be removed by DCE).
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("unused_ev".to_string()),
        vec![Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(Value::Number(99.0)),
        }],
    ));

    // Called procedure (should remain).
    stage.add_procedure(Procedure::new(
        "my_proc",
        vec![],
        vec![Stmt::ChangeVariable {
            var: "x".to_string(),
            delta: Expr::Literal(Value::Number(1.0)),
        }],
    ));

    // Uncalled procedure (should be removed by DCE).
    stage.add_procedure(Procedure::new(
        "unused_proc",
        vec![],
        vec![Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(Value::Number(0.0)),
        }],
    ));

    Project::new().with_stage(stage)
}

fn write_sb3_project(project: &scratcharch_scratchgraph::ir::Project, name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let sb3_path = dir.path().join(name);
    let writer = scratcharch_sb3::Sb3Writer::new();
    let archive = writer.write(project);
    let bytes = archive.to_bytes().unwrap();
    std::fs::write(&sb3_path, &bytes).unwrap();
    (dir, sb3_path)
}

#[test]
fn test_cli_optimize_sb3_roundtrip() {
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "opt_roundtrip.sb3");
    let out_path = std::env::temp_dir().join("cli_test_optimize_roundtrip_out.sb3");
    let _ = std::fs::remove_file(&out_path);

    let (ok, output) = run_cmd(&[
        "optimize",
        sb3_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert!(ok, "optimize roundtrip failed: {}", output);
    assert!(out_path.exists(), "output sb3 not created: {:?}", out_path);

    // Verify the output is a valid sb3 archive that can be read back.
    let bytes = std::fs::read(&out_path).unwrap();
    let archive = scratcharch_sb3::Sb3Archive::from_bytes(&bytes).unwrap();
    let project2 = scratcharch_sb3::Sb3Reader::new()
        .read(&archive)
        .expect("optimized sb3 should be readable");
    assert_eq!(project2.stage.name, "Stage");
}

#[test]
fn test_cli_optimize_dce() {
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "opt_dce.sb3");
    let out_path = std::env::temp_dir().join("cli_test_optimize_dce_out.sb3");
    let _ = std::fs::remove_file(&out_path);

    let (ok, output) = run_cmd(&[
        "optimize",
        sb3_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
        "--passes",
        "dce",
    ]);
    assert!(ok, "optimize dce failed: {}", output);

    let bytes = std::fs::read(&out_path).unwrap();
    let archive = scratcharch_sb3::Sb3Archive::from_bytes(&bytes).unwrap();
    let project2 = scratcharch_sb3::Sb3Reader::new()
        .read(&archive)
        .expect("optimized sb3 should be readable");

    // The unreachable broadcast script and uncalled procedure should be gone.
    assert_eq!(project2.stage.scripts.len(), 1, "green-flag script should remain");
    assert_eq!(project2.stage.procedures.len(), 1, "only my_proc should remain");
    assert_eq!(project2.stage.procedures[0].prototype.name, "my_proc");
}

#[test]
fn test_cli_optimize_constfold() {
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "opt_constfold.sb3");
    let out_path = std::env::temp_dir().join("cli_test_optimize_constfold_out.sb3");
    let _ = std::fs::remove_file(&out_path);

    let (ok, output) = run_cmd(&[
        "optimize",
        sb3_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
        "--passes",
        "constfold",
    ]);
    assert!(ok, "optimize constfold failed: {}", output);

    let bytes = std::fs::read(&out_path).unwrap();
    let archive = scratcharch_sb3::Sb3Archive::from_bytes(&bytes).unwrap();
    let project2 = scratcharch_sb3::Sb3Reader::new()
        .read(&archive)
        .expect("optimized sb3 should be readable");

    // 10 + 20 should be folded to 30 in the green-flag script.
    let body = &project2.stage.scripts[0].entry.body;
    if let scratcharch_scratchgraph::ir::Stmt::SetVariable { value, .. } = &body[0] {
        assert_eq!(
            value,
            &scratcharch_scratchgraph::ir::Expr::Literal(scratcharch_scratchgraph::ir::Value::Number(30.0)),
            "10+20 should fold to 30"
        );
    } else {
        panic!("expected SetVariable");
    }
}

#[test]
fn test_cli_optimize_report_json() {
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "opt_report.sb3");
    let out_path = std::env::temp_dir().join("cli_test_optimize_report_out.sb3");
    let report_path = std::env::temp_dir().join("cli_test_optimize_report.json");
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&report_path);

    let (ok, output) = run_cmd(&[
        "optimize",
        sb3_path.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
        "--format",
        "json",
        "--report",
        report_path.to_str().unwrap(),
    ]);
    assert!(ok, "optimize report json failed: {}", output);
    assert!(report_path.exists(), "report file not created");

    let report_text = std::fs::read_to_string(&report_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report_text).unwrap();
    let passes = report["passes"].as_array().expect("report should contain passes");
    assert!(!passes.is_empty(), "report should contain at least one pass");
}

#[test]
fn test_cli_verify_clean_project_passes() {
    // The optimizable fixture has foldable math, a provably-dead receiver, an
    // unreferenced variable, and dead/uncalled procedures. A *sound* optimizer
    // may remove those; verify must still PASS because each pass stays within
    // its contract.
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "verify_clean.sb3");

    let (ok, output) = run_cmd(&["verify", sb3_path.to_str().unwrap()]);
    assert!(ok, "verify clean project failed: {}", output);
    assert!(output.contains("Parse: PASS"), "output: {}", output);
    assert!(output.contains("Graph validation: PASS"), "output: {}", output);
    assert!(output.contains("Roundtrip: PASS"), "output: {}", output);
    assert!(output.contains("Semantic preservation: PASS"), "output: {}", output);
    assert!(output.contains("Transform preservation: PASS"), "output: {}", output);
}

#[test]
fn test_cli_verify_json_report() {
    let project = make_optimizable_project();
    let (_dir, sb3_path) = write_sb3_project(&project, "verify_json.sb3");

    let (ok, output) = run_cmd(&["verify", sb3_path.to_str().unwrap(), "--json"]);
    assert!(ok, "verify --json failed: {}", output);
    let report: serde_json::Value = serde_json::from_str(output.trim()).expect("valid json");
    assert_eq!(report["parse_ok"], true);
    assert_eq!(report["graph_valid"], true, "{}", output);
    assert_eq!(report["roundtrip_ok"], true, "{}", output);
    assert_eq!(report["semantic_preserved"], true, "{}", output);
    assert_eq!(report["transform_preserved"], true, "{}", output);
    let passes = report["passes"].as_array().expect("passes array present");
    assert_eq!(passes.len(), 4, "four default-pipeline passes are checked");
}

#[test]
fn test_cli_verify_flags_an_undeclared_variable_reference() {
    // The script references `ghost`, which no target declares: the project
    // parses and round-trips, but graph validation must fail.
    use scratcharch_scratchgraph::ir::*;
    let mut stage = Stage::new("Stage");
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::SetVariable {
            var: "ghost".to_string(),
            value: Expr::Literal(Value::Number(1.0)),
        }],
    ));
    let project = Project::new().with_stage(stage);
    let (_dir, sb3_path) = write_sb3_project(&project, "verify_ghost.sb3");

    let (ok, output) = run_cmd(&["verify", sb3_path.to_str().unwrap()]);
    assert!(!ok, "verify should fail on an undeclared reference: {}", output);
    assert!(output.contains("Graph validation: FAIL"), "output: {}", output);
    assert!(output.contains("ghost"), "output: {}", output);
}

#[test]
fn test_cli_verify_json_input_passes() {
    // `verify` also accepts a project.json directly (the non-sb3 load path);
    // the roundtrip leg still goes through real .sb3 bytes internally.
    let project = make_optimizable_project();
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("verify_input.json");
    use scratcharch_scratchgraph::{JsonExporter, ScratchExporter};
    let json = JsonExporter::new()
        .export(&project)
        .expect("export to json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let (ok, output) = run_cmd(&["verify", json_path.to_str().unwrap()]);
    assert!(ok, "verify json failed: {}", output);
    assert!(output.contains("Parse: PASS"), "output: {}", output);
    assert!(output.contains("Graph validation: PASS"), "output: {}", output);
    assert!(output.contains("Roundtrip: PASS"), "output: {}", output);
    assert!(output.contains("Semantic preservation: PASS"), "output: {}", output);
    assert!(output.contains("Transform preservation: PASS"), "output: {}", output);
}

#[test]
fn test_cli_verify_reports_unparseable_input() {
    let path = std::env::temp_dir().join("cli_test_verify_corrupt.sb3");
    std::fs::write(&path, b"this is not a zip archive").unwrap();

    let (ok, output) = run_cmd(&["verify", path.to_str().unwrap()]);
    assert!(!ok, "verify should fail on garbage input: {}", output);
    assert!(output.contains("Parse: FAIL"), "output: {}", output);
}

// ---- LLVM compatibility benchmark (test-compat) ----

fn run_compat_json(args: &[&str]) -> (bool, serde_json::Value, String) {
    let mut full: Vec<&str> = vec!["test-compat", "--no-native", "--format", "json"];
    full.extend_from_slice(args);
    let (ok, output) = run_cmd(&full);
    if ok {
        let value: serde_json::Value =
            serde_json::from_str(output.trim()).expect("test-compat emitted valid JSON");
        (ok, value, output)
    } else {
        (ok, serde_json::Value::Null, output)
    }
}

#[test]
fn test_cli_test_compat_json_gate_green() {
    // The corpus manifest lives at the workspace root; the CLI must discover it
    // from the crate directory without an explicit --corpus-root. The aggregate
    // global initializer (`global-agg`) now lays out into the static-data image,
    // so every `struct`-tagged fixture succeeds end-to-end through Scratch: the
    // feature is a clean 8/8 rather than the previous mixed distribution.
    let (ok, value, output) = run_compat_json(&["--feature", "struct"]);
    assert!(ok, "test-compat failed: {}", output);
    assert_eq!(value["gate_green"].as_bool(), Some(true), "output: {}", output);
    assert_eq!(value["total"].as_u64(), Some(8), "output: {}", output);
    assert_eq!(value["classes"]["success"].as_u64(), Some(8), "output: {}", output);
    assert_eq!(value["classes"]["parse-failure"].as_u64(), None, "output: {}", output);
    assert_eq!(value["stages"]["scratch"].as_u64(), Some(100), "output: {}", output);
}

#[test]
fn test_cli_test_compat_stage_filter_selects_parser_gaps() {
    let (ok, value, output) = run_compat_json(&["--stage", "parser"]);
    assert!(ok, "test-compat failed: {}", output);
    assert_eq!(value["total"].as_u64(), Some(4), "output: {}", output);
    assert_eq!(value["classes"]["parse-failure"].as_u64(), Some(4), "output: {}", output);
}

#[test]
fn test_cli_test_compat_quiet_reports_headline() {
    let (ok, output) = run_cmd(&[
        "test-compat",
        "--feature",
        "struct",
        "--no-native",
        "--quiet",
    ]);
    assert!(ok, "test-compat failed: {}", output);
    assert!(output.contains("Overall"), "quiet must show the headline: {}", output);
    assert!(output.contains("Gate: green"), "quiet must show the gate: {}", output);
}

#[test]
fn test_cli_test_compat_unknown_feature_fails() {
    let (ok, output) = run_cmd(&[
        "test-compat",
        "--feature",
        "bogus",
        "--no-native",
        "--quiet",
    ]);
    assert!(!ok, "unknown feature must be rejected: {}", output);
    assert!(output.contains("unknown feature"), "output: {}", output);
}

#[test]
fn test_cli_test_compat_quiet_and_verbose_conflict() {
    let (ok, output) = run_cmd(&[
        "test-compat",
        "--feature",
        "struct",
        "--no-native",
        "--quiet",
        "--verbose",
    ]);
    assert!(!ok, "--quiet and --verbose must conflict: {}", output);
}
