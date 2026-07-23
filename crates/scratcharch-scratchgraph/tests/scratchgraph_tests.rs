//! Tests for the SAIR → ScratchGraph → Exporter pipeline.
//!
//! These tests focus on architecture rather than full Scratch compatibility.
//! They verify that SAIR modules lower to semantically plausible ScratchGraph
//! projects and that the Scratch 3 JSON exporter emits a valid project.json.

use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::types::IrType;
use scratcharch_scratchgraph::{
    Expr, Hat, JsonExporter, List, ListScope, Project, ScratchExporter, ScratchGraphLowerer,
    Script, Sprite, Stage, Stmt, StopOption, Value, Variable, VariableScope,
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

// ── Event hats ───────────────────────────────────────────────────

#[test]
fn test_scratchgraph_event_hats() {
    let mut stage = Stage::new("Stage");
    stage.add_script(Script::new(Hat::GreenFlag, vec![]));
    stage.add_script(Script::new(Hat::KeyPressed("space".to_string()), vec![]));
    stage.add_script(Script::new(Hat::SpriteClicked, vec![]));
    stage.add_script(Script::new(
        Hat::BroadcastReceived("hello".to_string()),
        vec![],
    ));
    stage.add_script(Script::new(Hat::CloneStart, vec![]));

    let project = Project::new().with_stage(stage);
    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    let opcodes: Vec<_> = blocks
        .values()
        .map(|b| b["opcode"].as_str().unwrap())
        .collect();
    assert!(opcodes.contains(&"event_whenflagclicked"));
    assert!(opcodes.contains(&"event_whenkeypressed"));
    assert!(opcodes.contains(&"event_whenthisspriteclicked"));
    assert!(opcodes.contains(&"event_whenbroadcastreceived"));
    assert!(opcodes.contains(&"event_whencloned"));
}

// ── Variable scopes ──────────────────────────────────────────────

#[test]
fn test_scratchgraph_variable_scopes() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(Variable::new("g", "global"));

    let mut sprite = Sprite::new("Sprite1");
    sprite.add_variable(
        Variable::new("l", "local").with_scope(VariableScope::SpriteLocal),
    );
    sprite.add_variable(
        Variable::new("t", "tmp").with_scope(VariableScope::Temporary),
    );

    let mut project = Project::new().with_stage(stage);
    project.add_sprite(sprite);

    assert_eq!(project.stage.variables[0].scope, VariableScope::Global);
    assert_eq!(
        project.sprites[0].variables[0].scope,
        VariableScope::SpriteLocal
    );
    assert_eq!(
        project.sprites[0].variables[1].scope,
        VariableScope::Temporary
    );

    let json = export_json(&project);
    let stage_vars = json["targets"][0]["variables"].as_object().unwrap();
    let sprite_vars = json["targets"][1]["variables"].as_object().unwrap();
    assert!(stage_vars.contains_key("g"));
    assert!(sprite_vars.contains_key("l"));
    assert!(sprite_vars.contains_key("t"));
}

// ── List operations ──────────────────────────────────────────────

#[test]
fn test_scratchgraph_list_operations() {
    let mut stage = Stage::new("Stage");
    stage.add_list(List::new("nums", "nums"));
    stage.add_list(List::new("local", "local").with_scope(ListScope::SpriteLocal));

    let body = vec![
        Stmt::AddToList {
            list: "nums".to_string(),
            value: Expr::number(1.0),
        },
        Stmt::SetListItem {
            list: "nums".to_string(),
            index: Expr::number(1.0),
            value: Expr::number(42.0),
        },
        Stmt::InsertListItem {
            list: "nums".to_string(),
            index: Expr::number(2.0),
            value: Expr::number(99.0),
        },
        Stmt::DeleteListItem {
            list: "nums".to_string(),
            index: Expr::number(3.0),
        },
        Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::ListItem {
                list: "nums".to_string(),
                index: Box::new(Expr::number(1.0)),
            },
        },
        Stmt::SetVariable {
            var: "len".to_string(),
            value: Expr::list_length("nums"),
        },
    ];
    stage.add_script(Script::new(Hat::GreenFlag, body));

    let project = Project::new().with_stage(stage);
    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    let opcodes: Vec<_> = blocks
        .values()
        .map(|b| b["opcode"].as_str().unwrap())
        .collect();
    assert!(opcodes.contains(&"data_addtolist"));
    assert!(opcodes.contains(&"data_replaceitemoflist"));
    assert!(opcodes.contains(&"data_insertatlist"));
    assert!(opcodes.contains(&"data_deleteoflist"));
    assert!(opcodes.contains(&"data_itemoflist"));
    assert!(opcodes.contains(&"data_lengthoflist"));
}

// ── Procedure return values ──────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_return_value() {
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
    let result = builder.call(IrType::I32, "add", vec![one, two]).expect("call result");
    builder.ret(Some(result));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let add_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "add")
        .expect("add procedure missing");
    assert_eq!(add_proc.return_var, Some("__ret_add".to_string()));

    // The callee should write the hidden return variable before stopping.
    let add_body_has_ret_set = add_proc
        .body
        .iter()
        .any(|s| matches!(s, Stmt::SetVariable { var, .. } if var == "__ret_add"));
    assert!(add_body_has_ret_set);

    // The caller should copy the hidden variable into its result SSA value.
    let main_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "main")
        .expect("main procedure missing");
    let main_body_has_ret_read = main_proc.body.iter().any(|s| matches!(
        s,
        Stmt::SetVariable {
            value: Expr::Variable(v),
            ..
        } if v == "__ret_add"
    ));
    assert!(main_body_has_ret_read);

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_setvariableto"));
}

// ── Multi-script sprite ──────────────────────────────────────────

#[test]
fn test_scratchgraph_multi_script_sprite() {
    let stage = Stage::new("Stage");

    let mut sprite = Sprite::new("Sprite1");
    sprite.add_script(Script::new(Hat::GreenFlag, vec![Stmt::Stop {
        option: StopOption::ThisScript,
    }]));
    sprite.add_script(Script::new(
        Hat::KeyPressed("space".to_string()),
        vec![Stmt::Stop {
            option: StopOption::ThisScript,
        }],
    ));

    let mut project = Project::new().with_stage(stage);
    project.add_sprite(sprite);

    assert_eq!(project.sprites[0].scripts.len(), 2);

    let json = export_json(&project);
    let sprite_json = &json["targets"][1];
    let blocks = sprite_json["blocks"].as_object().unwrap();
    let hat_count = blocks
        .values()
        .filter(|b| {
            let opcode = b["opcode"].as_str().unwrap();
            opcode == "event_whenflagclicked" || opcode == "event_whenkeypressed"
        })
        .count();
    assert_eq!(hat_count, 2);
}

// ── Nested conditional inside loop ─────────────────────────────────

#[test]
fn test_sair_nested_conditional_inside_loop() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::Void);
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let cond = builder.eq(IrType::I32, zero, one);
    builder.cond_br(cond, "exit", "body");

    builder.new_block("body");
    let cond2 = builder.eq(IrType::I32, zero, one);
    builder.cond_br(cond2, "then", "else_");

    builder.new_block("then");
    builder.br("merge");

    builder.new_block("else_");
    builder.br("merge");

    builder.new_block("merge");
    builder.br("entry");

    builder.new_block("exit");
    builder.ret(None);

    let project = lower(&builder.finish());
    let stage = stage(&project);

    // The outer loop should be recognized and the body should contain an If.
    let main_proc = &stage.procedures[0];
    assert!(main_proc.body.iter().any(|s| matches!(
        s,
        Stmt::RepeatUntil { body, .. } if body.iter().any(|inner| matches!(inner, Stmt::If { .. }))
    )));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "control_repeat_until"));
    assert!(blocks.values().any(|b| b["opcode"] == "control_if_else"));
}

// ── Heap memory lowering ─────────────────────────────────────────

#[test]
fn test_sair_to_scratchgraph_heap_memory() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let addr = builder.alloca(IrType::I32);
    let value = builder.const_i32(42);
    builder.store(IrType::I32, value, addr);
    let loaded = builder.load(IrType::I32, addr);
    builder.ret(Some(loaded));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    // The stage should declare the heap list.
    assert!(stage
        .lists
        .iter()
        .any(|l| l.name == "__scratcharch_heap"));

    // The procedure body should contain heap list operations.
    let body = &stage.procedures[0].body;
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::SetListItem { list, .. } if list == "__scratcharch_heap"
    )));
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::SetVariable { value, .. } if matches!(value, Expr::HeapLoad { .. })
    )));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_replaceitemoflist"));
    assert!(blocks.values().any(|b| b["opcode"] == "data_itemoflist"));
}
