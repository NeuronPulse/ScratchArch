//! Tests for the SAIR → ScratchGraph → Exporter pipeline.
//!
//! These tests focus on architecture rather than full Scratch compatibility.
//! They verify that SAIR modules lower to semantically plausible ScratchGraph
//! projects and that the Scratch 3 JSON exporter emits a valid project.json.

use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::types::IrType;
use scratcharch_scratchgraph::{
    Expr, Hat, JsonExporter, Project, ScratchExporter, ScratchGraphLowerer, Stage, Stmt, StopOption,
    Value,
};

fn lower(module: &scratcharch_ir::r#module::IrModule) -> Project {
    ScratchGraphLowerer::new().lower(module).expect("lowering failed")
}

fn export_json(project: &Project) -> serde_json::Value {
    JsonExporter::new().export(project).expect("export failed")
}

fn stage(project: &Project) -> &Stage {
    &project.stage
}

// ── Arithmetic ───────────────────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_arithmetic() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let sum = builder.add(IrType::I32, a, b);
    builder.ret(Some(sum));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    assert_eq!(stage.procedures.len(), 1);
    assert_eq!(stage.procedures[0].prototype.name, "main");

    // Body should set a variable to the result of an add operator and stop.
    let body = &stage.procedures[0].body;
    assert!(body.len() >= 2);
    assert!(matches!(
        body.last(),
        Some(Stmt::Stop {
            option: StopOption::ThisScript
        })
    ));

    let json = export_json(&project);
    assert!(!json.get("targets").unwrap().as_array().unwrap().is_empty());
    let stage_json = &json["targets"][0];
    assert!(!stage_json["blocks"].as_object().unwrap().is_empty());
}

// ── Variable assignment ──────────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_variable_assignment() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let value = builder.const_i32(42);
    builder.ret(Some(value));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    // The constant should lower to a SetVariable statement.
    let body = &stage.procedures[0].body;
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::SetVariable {
            value: Expr::Literal(Value::Number(42.0)),
            ..
        }
    )));

    // The stage should declare the SSA variable.
    assert!(!stage.variables.is_empty());

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_setvariableto"));
}

// ── Conditional ──────────────────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_conditional() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let cond = builder.eq(IrType::I32, zero, one);
    builder.cond_br(cond, "then", "else_");

    builder.new_block("then");
    let v42 = builder.const_i32(42);
    builder.br("end");

    builder.new_block("else_");
    let v7 = builder.const_i32(7);
    builder.br("end");

    builder.new_block("end");
    let result = builder.phi(IrType::I32, vec![(v42, "then"), (v7, "else_")]);
    builder.ret(Some(result));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let body = &stage.procedures[0].body;
    assert!(body.iter().any(|s| matches!(s, Stmt::If { .. })));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "control_if_else"));
}

// ── Procedure call ───────────────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_procedure_call() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("add", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    let sum = builder.add(IrType::I32, a, b);
    builder.ret(Some(sum));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let one = builder.const_i32(20);
    let two = builder.const_i32(22);
    let _result = builder.call(IrType::I32, "add", vec![one, two]);
    builder.ret(None);

    let project = lower(&builder.finish());
    let stage = stage(&project);

    assert_eq!(stage.procedures.len(), 2);
    let proc_names: Vec<_> = stage
        .procedures
        .iter()
        .map(|p| p.prototype.name.clone())
        .collect();
    assert!(proc_names.contains(&"add".to_string()));
    assert!(proc_names.contains(&"main".to_string()));

    // The entry script should call the entry procedure.
    assert_eq!(stage.scripts.len(), 1);
    assert!(matches!(stage.scripts[0].hat, Hat::GreenFlag));
    assert!(stage.scripts[0]
        .body
        .iter()
        .any(|s| matches!(s, Stmt::Call { proc, .. } if proc == "main")));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "procedures_call"));
    assert!(blocks.values().any(|b| b["opcode"] == "procedures_definition"));
}

// ── Loop (exporter path) ─────────────────────────────────────────
// SAIR→ScratchGraph loop lowering is intentionally limited in v0.1.
// This test verifies the exporter can emit a ScratchGraph `repeat until`
// constructed from the IR API.

#[test]
fn test_scratchgraph_loop_exporter() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(scratcharch_scratchgraph::Variable::new("i", "i"));

    let body = vec![
        Stmt::SetVariable {
            var: "i".to_string(),
            value: Expr::number(0.0),
        },
        Stmt::RepeatUntil {
            condition: Expr::Operator {
                opcode: "operator_gt".to_string(),
                args: vec![Expr::variable("i"), Expr::number(5.0)],
            },
            body: vec![Stmt::ChangeVariable {
                var: "i".to_string(),
                delta: Expr::number(1.0),
            }],
        },
        Stmt::Stop {
            option: StopOption::ThisScript,
        },
    ];
    stage.add_script(scratcharch_scratchgraph::Script::new(Hat::GreenFlag, body));

    let project = Project::new().with_stage(stage);
    let json = export_json(&project);

    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "control_repeat_until"));
    assert!(blocks.values().any(|b| b["opcode"] == "data_changevariableby"));
}

// ── Loop (SAIR lowering, limited) ─────────────────────────────────
// The current lowering treats simple self-loop SAIR patterns as valid input
// and emits a ScratchGraph project without crashing. This test guards the
// pipeline against regressions while loop recognition matures.

#[test]
fn test_sair_loop_lowering_does_not_crash() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let cond = builder.eq(IrType::I32, zero, one);
    builder.cond_br(cond, "exit", "body");

    builder.new_block("body");
    // Body does nothing meaningful and jumps back to entry.
    builder.br("entry");

    builder.new_block("exit");
    builder.ret(Some(zero));

    let module = builder.finish();
    let project = lower(&module);
    let _json = export_json(&project);
}
