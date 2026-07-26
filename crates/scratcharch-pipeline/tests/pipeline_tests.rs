use scratcharch_pipeline::{Diagnostic, DiagnosticLevel, DiagnosticSink, Pipeline, PipelineConfig};

fn minimal_project_json() -> serde_json::Value {
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
}

fn simple_sair_module() -> String {
    r#"sair 0.1
entry "main"

func @main -> i32 entry "entry" {
  block "entry":
    %0 = const i32 42
    ret i32 %0
}
"#
    .to_string()
}

#[test]
fn test_pipeline_llvm_to_json() {
    let llvm = r#"
define i32 @main() {
  ret i32 42
}
"#;
    let mut pipeline = Pipeline::new(PipelineConfig::default());
    let result = pipeline.run_llvm_to_json(llvm);
    assert!(result.is_ok(), "pipeline failed: {:?}", result.err());
}

#[test]
fn test_pipeline_roundtrip_empty_project() {
    let json = minimal_project_json();
    let mut pipeline = Pipeline::new(PipelineConfig::default());
    let result = pipeline.run_json_roundtrip(&json);
    assert!(result.is_ok(), "roundtrip failed: {:?}", result.err());
}

#[test]
fn test_pipeline_decompile_empty() {
    let json = minimal_project_json();
    let mut pipeline = Pipeline::new(PipelineConfig::default());
    let result = pipeline.run_decompile(&json);
    assert!(result.is_ok(), "decompile failed: {:?}", result.err());
    let result = result.unwrap();
    match result.output {
        scratcharch_pipeline::PipelineOutput::SairText(text) => {
            assert!(text.contains("sair 0.1"), "missing header: {}", text);
        }
        _ => panic!("expected SAIR output"),
    }
}

#[test]
fn test_pipeline_diagnostics() {
    let mut sink = DiagnosticSink::new();

    sink.push_error("undefined variable x");
    assert!(sink.has_errors());

    sink.push_warning("unused variable y");
    sink.push_note("declared at line 10");

    let text = sink.to_text();
    assert!(text.contains("error: undefined variable x"));
    assert!(text.contains("warning: unused variable y"));
    assert!(text.contains("note: declared at line 10"));
}

#[test]
fn test_diagnostic_with_source_location() {
    let diag = Diagnostic::error("undefined variable x")
        .with_stage("analyze")
        .with_source("sprite1/main", 10, 5)
        .with_suggestion("check variable scope");

    let text = diag.to_string();
    assert!(text.contains("error: undefined variable x"));
    assert!(text.contains("[analyze]"));
    assert!(text.contains("sprite1/main:10:5"));
    assert!(text.contains("check variable scope"));
}

#[test]
fn test_diagnostic_json_output() {
    let mut sink = DiagnosticSink::new();
    sink.push(Diagnostic::error("test error").with_suggestion("fix it"));

    let json = sink.to_json().unwrap();
    assert!(json.contains("\"level\": \"Error\""));
    assert!(json.contains("\"message\": \"test error\""));
    assert!(json.contains("\"suggestion\": \"fix it\""));
}

#[test]
fn test_decompile_with_procedure() {
    use scratcharch_scratchgraph::ir::{
        EventHat, Expr, Procedure, Project, Script, Stage, Stmt, Value as SgValue,
    };

    let mut project = Project::new();
    project.stage = Stage::new("Stage");

    project.stage.procedures.push(Procedure::new(
        "add",
        vec![],
        vec![
            Stmt::SetVariable {
                var: "x".to_string(),
                value: Expr::Literal(SgValue::Number(1.0)),
            },
            Stmt::SetVariable {
                var: "y".to_string(),
                value: Expr::Literal(SgValue::Number(2.0)),
            },
        ],
    ));

    let decompiler = scratcharch_pipeline::decompile::Decompiler::new();
    let module = decompiler.decompile(&project).expect("decompile failed");
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.functions[0].name, "add");
}

#[test]
fn test_pipeline_invalid_input() {
    let mut pipeline = Pipeline::new(PipelineConfig::default());
    let result = pipeline.run_llvm_to_json("not valid llvm");
    assert!(result.is_err());
}
