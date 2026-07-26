use scratcharch_explorer::sair::SairExplorer;
use scratcharch_explorer::scratch::ScratchExplorer;

/// Helper: create a minimal SAIR module via the builder.
fn make_sair_module() -> scratcharch_ir::module::IrModule {
    use scratcharch_ir::builder::IrBuilder;
    use scratcharch_ir::types::IrType;
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let v = builder.const_i32(42);
    builder.ret(Some(v));
    builder.finish()
}

#[test]
fn test_explorer_sair_basic() {
    let module = make_sair_module();
    let explorer = SairExplorer::new();
    let summary = explorer.explore_module(&module);
    assert_eq!(summary.entry, "main");
    assert_eq!(summary.function_count, 1);
    assert_eq!(summary.total_instructions, 1);
    assert_eq!(summary.total_blocks, 1);
}

#[test]
fn test_explorer_sair_text_output() {
    let module = make_sair_module();
    let explorer = SairExplorer::new();
    let text = explorer.to_text(&module);
    assert!(text.contains("SAIR Module"));
    assert!(text.contains("main"));
    assert!(text.contains("entry"));
}

#[test]
fn test_explorer_sair_json_output() {
    let module = make_sair_module();
    let explorer = SairExplorer::new();
    let json = explorer.to_json(&module).unwrap();
    assert!(json.contains("\"entry\": \"main\""));
    assert!(json.contains("\"function_count\": 1"));
}

#[test]
fn test_explorer_scratch_basic() {
    let mut project = scratcharch_scratchgraph::ir::Project::new();
    project.stage.name = "TestProject".to_string();
    project.stage.scripts.push(
        scratcharch_scratchgraph::ir::Script::new(
            scratcharch_scratchgraph::ir::EventHat::GreenFlag,
            vec![
                scratcharch_scratchgraph::ir::Stmt::SetVariable {
                    var: "x".to_string(),
                    value: scratcharch_scratchgraph::ir::Expr::number(1.0),
                },
            ],
        ),
    );
    project.stage.procedures.push(
        scratcharch_scratchgraph::ir::Procedure::new(
            "my_proc",
            vec![],
            vec![],
        ),
    );

    let explorer = ScratchExplorer::new();
    let summary = explorer.explore_project(&project);
    assert_eq!(summary.project_name, "TestProject");
    assert_eq!(summary.sprite_count, 0);
    assert_eq!(summary.targets.len(), 1);
    assert_eq!(summary.targets[0].script_count, 1);
    assert_eq!(summary.targets[0].procedure_count, 1);
}

#[test]
fn test_explorer_scratch_json_output() {
    let mut project = scratcharch_scratchgraph::ir::Project::new();
    project.stage.name = "JSONTest".to_string();

    let explorer = ScratchExplorer::new();
    let json = explorer.to_json(&project).unwrap();
    assert!(json.contains("\"project_name\": \"JSONTest\""));
    assert!(json.contains("\"targets\""));
}

#[test]
fn test_explorer_scratch_text_output() {
    let mut project = scratcharch_scratchgraph::ir::Project::new();
    project.stage.name = "TextTest".to_string();

    let explorer = ScratchExplorer::new();
    let text = explorer.to_text(&project);
    assert!(text.contains("TextTest"));
    assert!(text.contains("Stage"));
}
