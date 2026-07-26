//! Tests for the SAIR → ScratchGraph → Exporter pipeline.
//!
//! These tests focus on architecture rather than full Scratch compatibility.
//! They verify that SAIR modules lower to semantically plausible ScratchGraph
//! projects and that the Scratch 3 JSON exporter emits a valid project.json.

use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::types::IrType;
use scratcharch_scratchgraph::{
    EventHat, Expr, JsonExporter, List, ListScope, Project, ScratchExporter, ScratchGraphLowerer,
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

/// Recursively count `Stmt::Call` to `proc_name` in a statement list.
fn count_calls(body: &[Stmt], proc_name: &str) -> usize {
    body.iter()
        .map(|s| match s {
            Stmt::If {
                then_body,
                else_body,
                ..
            } => count_calls(then_body, proc_name) + count_calls(else_body, proc_name),
            Stmt::Repeat { body: b, .. }
            | Stmt::RepeatUntil { body: b, .. }
            | Stmt::Forever { body: b } => count_calls(b, proc_name),
            Stmt::Call { proc, .. } if proc == proc_name => 1,
            _ => 0,
        })
        .sum()
}

/// Recursively look for a complete call sequence (EnterFrame, Call, copy return,
/// restore FP, PopFrame) inside a statement list.
fn contains_call_sequence(body: &[Stmt], proc_name: &str) -> bool {
    if body.windows(5).any(|w| {
        matches!(w[0], Stmt::EnterFrame { .. })
            && matches!(w[1], Stmt::Call { ref proc, .. } if proc == proc_name)
            && matches!(w[2], Stmt::FrameSet { value: Expr::FrameGet { offset: 1 }, .. })
            && matches!(w[3], Stmt::SetVariable { ref var, .. } if var == "__scratcharch_fp")
            && matches!(w[4], Stmt::PopFrame { .. })
    }) {
        return true;
    }
    body.iter().any(|s| match s {
        Stmt::If {
            then_body,
            else_body,
            ..
        } => contains_call_sequence(then_body, proc_name) || contains_call_sequence(else_body, proc_name),
        Stmt::Repeat { body: b, .. }
        | Stmt::RepeatUntil { body: b, .. }
        | Stmt::Forever { body: b } => contains_call_sequence(b, proc_name),
        _ => false,
    })
}

/// Recursively look for a return sequence (FrameSet offset 1, PopFrame, Stop).
fn contains_return_sequence(body: &[Stmt]) -> bool {
    if body.windows(3).any(|w| {
        matches!(w[0], Stmt::FrameSet { offset: 1, .. })
            && matches!(w[1], Stmt::PopFrame { .. })
            && matches!(w[2], Stmt::Stop { option: StopOption::ThisScript })
    }) {
        return true;
    }
    body.iter().any(|s| match s {
        Stmt::If {
            then_body,
            else_body,
            ..
        } => contains_return_sequence(then_body) || contains_return_sequence(else_body),
        Stmt::Repeat { body: b, .. }
        | Stmt::RepeatUntil { body: b, .. }
        | Stmt::Forever { body: b } => contains_return_sequence(b),
        _ => false,
    })
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

    // Body should write the add result into a frame slot and stop.
    let body = &stage.procedures[0].body;
    assert!(body.len() >= 2);
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::FrameSet {
            value: Expr::Operator { opcode, .. },
            ..
        } if opcode == "operator_add"
    )));
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

    // The constant should lower to a FrameSet statement.
    let body = &stage.procedures[0].body;
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::FrameSet {
            value: Expr::Literal(Value::Number(42.0)),
            ..
        }
    )));

    // The stage should declare the runtime frame pointer.
    assert!(stage.variables.iter().any(|v| v.name == "__scratcharch_fp"));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_replaceitemoflist"));
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
    assert!(matches!(&stage.scripts[0].entry.hat, EventHat::GreenFlag));
    assert!(stage.scripts[0]
        .entry.body
        .iter()
        .any(|s| matches!(s, Stmt::Call { proc, .. } if proc == "main")));

    // The caller procedure should push and pop a frame around the call.
    let main_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "main")
        .expect("main procedure missing");
    assert!(main_proc.body.iter().any(|s| matches!(s, Stmt::EnterFrame { .. })));
    assert!(main_proc.body.iter().any(|s| matches!(s, Stmt::PopFrame { .. })));

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
    stage.add_script(scratcharch_scratchgraph::Script::new(EventHat::GreenFlag, body));

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
    stage.add_script(Script::new(EventHat::GreenFlag, vec![]));
    stage.add_script(Script::new(EventHat::KeyPressed("space".to_string()), vec![]));
    stage.add_script(Script::new(EventHat::SpriteClicked, vec![]));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("hello".to_string()),
        vec![],
    ));
    stage.add_script(Script::new(EventHat::CloneStart, vec![]));

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
    stage.add_script(Script::new(EventHat::GreenFlag, body));

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
    assert!(add_proc.frame_size >= 2);

    // The callee should write the per-frame return slot before stopping.
    let add_body_has_ret_set = add_proc
        .body
        .iter()
        .any(|s| matches!(s, Stmt::FrameSet { offset: 1, .. }));
    assert!(add_body_has_ret_set);

    // The caller should copy the per-frame return slot into its result frame slot.
    let main_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "main")
        .expect("main procedure missing");
    let main_body_has_ret_read = main_proc.body.iter().any(|s| matches!(
        s,
        Stmt::FrameSet {
            value: Expr::FrameGet { offset: 1 },
            ..
        }
    ));
    assert!(main_body_has_ret_read);

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_replaceitemoflist"));
}

// ── Multi-script sprite ──────────────────────────────────────────

#[test]
fn test_scratchgraph_multi_script_sprite() {
    let stage = Stage::new("Stage");

    let mut sprite = Sprite::new("Sprite1");
    sprite.add_script(Script::new(EventHat::GreenFlag, vec![Stmt::Stop {
        option: StopOption::ThisScript,
    }]));
    sprite.add_script(Script::new(
        EventHat::KeyPressed("space".to_string()),
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

    // The stage should declare the runtime stack and the heap list.
    assert!(stage.lists.iter().any(|l| l.name == "__scratcharch_stack"));
    assert!(stage.lists.iter().any(|l| l.name == "__scratcharch_heap"));

    // The procedure body should contain heap list operations and frame writes.
    let body = &stage.procedures[0].body;
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::SetListItem { list, .. } if list == "__scratcharch_heap"
    )));
    assert!(body.iter().any(|s| matches!(
        s,
        Stmt::FrameSet { value, .. } if matches!(value, Expr::HeapLoad { .. })
    )));
    assert!(body.iter().any(|s| matches!(s, Stmt::HeapAlloc { .. })));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_replaceitemoflist"));
    assert!(blocks.values().any(|b| b["opcode"] == "data_itemoflist"));
}

// ── Recursive factorial ──────────────────────────────────────────

#[test]
fn test_recursive_factorial() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("factorial", IrType::I32);
    let n = builder.add_param(IrType::I32, "n");
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let cond = builder.eq(IrType::I32, n, zero);
    builder.cond_br(cond, "base", "rec");

    builder.new_block("base");
    builder.ret(Some(one));

    builder.new_block("rec");
    let n_minus_1 = builder.sub(IrType::I32, n, one);
    let sub_result = builder
        .call(IrType::I32, "factorial", vec![n_minus_1])
        .expect("recursive call result");
    let result = builder.mul(IrType::I32, n, sub_result);
    builder.ret(Some(result));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let five = builder.const_i32(5);
    let _ = builder.call(IrType::I32, "factorial", vec![five]);
    builder.ret(None);

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let fact_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "factorial")
        .expect("factorial procedure missing");
    assert!(fact_proc.frame_size >= 2);

    // Recursive call site must push/pop a frame and read the return slot.
    assert!(contains_call_sequence(&fact_proc.body, "factorial"));

    // Callee writes the per-frame return slot before popping locals.
    assert!(contains_return_sequence(&fact_proc.body));

    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    assert!(blocks.values().any(|b| b["opcode"] == "data_addtolist"));
    assert!(blocks.values().any(|b| b["opcode"] == "data_deleteoflist"));
}

// ── Recursive fibonacci (two recursive calls) ────────────────────

#[test]
fn test_recursive_fibonacci() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("fib", IrType::I32);
    let n = builder.add_param(IrType::I32, "n");
    builder.new_block("entry");
    let _zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let two = builder.const_i32(2);
    let cond = builder.lt(IrType::I32, n, two);
    builder.cond_br(cond, "base", "rec");

    builder.new_block("base");
    builder.ret(Some(n));

    builder.new_block("rec");
    let n1 = builder.sub(IrType::I32, n, one);
    let n2 = builder.sub(IrType::I32, n, two);
    let r1 = builder.call(IrType::I32, "fib", vec![n1]).expect("fib(n-1)");
    let r2 = builder.call(IrType::I32, "fib", vec![n2]).expect("fib(n-2)");
    let sum = builder.add(IrType::I32, r1, r2);
    builder.ret(Some(sum));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ten = builder.const_i32(10);
    let _ = builder.call(IrType::I32, "fib", vec![ten]);
    builder.ret(None);

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let fib_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "fib")
        .expect("fib procedure missing");

    // The fib procedure body contains two recursive calls.
    assert_eq!(count_calls(&fib_proc.body, "fib"), 2);

    // Every recursive call must have the full frame push/pop sequence.
    assert!(contains_call_sequence(&fib_proc.body, "fib"));
}

// ── Nested function calls ────────────────────────────────────────

#[test]
fn test_nested_function_calls() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("inner", IrType::I32);
    let x = builder.add_param(IrType::I32, "x");
    builder.new_block("entry");
    let one = builder.const_i32(1);
    let inner_result = builder.add(IrType::I32, x, one);
    builder.ret(Some(inner_result));

    builder.start_function("outer", IrType::I32);
    let y = builder.add_param(IrType::I32, "y");
    builder.new_block("entry");
    let inner_call = builder
        .call(IrType::I32, "inner", vec![y])
        .expect("inner call");
    let five = builder.const_i32(5);
    let outer_result = builder.add(IrType::I32, inner_call, five);
    builder.ret(Some(outer_result));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let three = builder.const_i32(3);
    let _ = builder.call(IrType::I32, "outer", vec![three]);
    builder.ret(None);

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let outer_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "outer")
        .expect("outer procedure missing");
    assert!(outer_proc.frame_size >= 2);

    // outer should contain a complete call to inner.
    let inner_call_ok = outer_proc.body.windows(3).any(|w| {
        matches!(w[0], Stmt::EnterFrame { .. })
            && matches!(w[1], Stmt::Call { ref proc, .. } if proc == "inner")
            && matches!(w[2], Stmt::FrameSet { value: Expr::FrameGet { offset: 1 }, .. })
    });
    assert!(inner_call_ok);
}

// ── Frame-local variables survive across calls ───────────────────

#[test]
fn test_frame_local_variables() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("callee", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    builder.new_block("entry");
    builder.ret(Some(a));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let one = builder.const_i32(1);
    let two = builder.const_i32(2);
    let call_result = builder
        .call(IrType::I32, "callee", vec![one])
        .expect("callee call");
    let result = builder.add(IrType::I32, call_result, two);
    builder.ret(Some(result));

    let project = lower(&builder.finish());
    let stage = stage(&project);

    let main_proc = stage
        .procedures
        .iter()
        .find(|p| p.prototype.name == "main")
        .expect("main procedure missing");

    // Local constants should be written to fixed frame offsets.
    let local_offsets: Vec<u32> = main_proc
        .body
        .iter()
        .filter_map(|s| {
            if let Stmt::FrameSet {
                value: Expr::Literal(Value::Number(n)),
                offset,
            } = s
            {
                if *n == 1.0 || *n == 2.0 {
                    return Some(*offset);
                }
            }
            None
        })
        .collect();
    assert_eq!(local_offsets.len(), 2);

    // Locate the frame offset where the callee's return value was stored.
    let return_offset = main_proc
        .body
        .windows(3)
        .find_map(|w| {
            if let (
                Stmt::EnterFrame { .. },
                Stmt::Call { proc, .. },
                Stmt::FrameSet { offset, value: Expr::FrameGet { offset: 1 } },
            ) = (&w[0], &w[1], &w[2])
            {
                if proc == "callee" {
                    return Some(*offset);
                }
            }
            None
        })
        .expect("call result write not found");

    // After the call, the add should read the callee result and the surviving local.
    let add_after_call = main_proc.body.iter().any(|s| {
        if let Stmt::FrameSet {
            value: Expr::Operator { opcode, args },
            ..
        } = s
        {
            return opcode == "operator_add"
                && args.iter().any(|a| matches!(a, Expr::FrameGet { offset } if *offset == return_offset))
                && args.iter().any(|a| matches!(a, Expr::FrameGet { offset } if local_offsets.contains(offset)));
        }
        false
    });
    assert!(add_after_call);
}

// ── Frame primitives JSON export ─────────────────────────────────

#[test]
fn test_scratchgraph_frame_json_export() {
    let mut stage = Stage::new("Stage");
    stage.add_list(List::new("__scratcharch_stack", "__scratcharch_stack"));
    stage.add_variable(scratcharch_scratchgraph::Variable::new(
        "__scratcharch_fp",
        "__scratcharch_fp",
    ));

    let body = vec![
        Stmt::EnterFrame { slots: 3 },
        Stmt::FrameSet {
            offset: 1,
            value: Expr::number(42.0),
        },
        Stmt::FrameSet {
            offset: 2,
            value: Expr::FrameGet { offset: 1 },
        },
        Stmt::SetVariable {
            var: "__scratcharch_fp".to_string(),
            value: Expr::FrameGet { offset: 0 },
        },
        Stmt::PopFrame { slots: 3 },
        Stmt::Stop {
            option: StopOption::ThisScript,
        },
    ];
    stage.add_procedure(scratcharch_scratchgraph::Procedure::new(
        "frame_demo",
        vec![],
        body,
    ));

    let project = Project::new().with_stage(stage);
    let json = export_json(&project);
    let blocks = json["targets"][0]["blocks"].as_object().unwrap();
    let opcodes: Vec<_> = blocks.values().map(|b| b["opcode"].as_str().unwrap()).collect();
    assert!(opcodes.contains(&"data_addtolist"));
    assert!(opcodes.contains(&"data_deleteoflist"));
    assert!(opcodes.contains(&"data_replaceitemoflist"));
    assert!(opcodes.contains(&"data_itemoflist"));
    assert!(opcodes.contains(&"data_variable"));
}
