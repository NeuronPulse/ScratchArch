//! Scratch 3 JSON exporter for ScratchGraph.
//!
//! Translates a semantic `Project` into the `project.json` format accepted by
//! the Scratch 3 offline editor and the open-source Scratch VM.

use serde_json::{json, Map, Value};

use crate::exporter::{Infallible, ScratchExporter};
use crate::ir::{
    Expr, Hat, Procedure, Project, Script, Stage, Stmt, StopOption, Value as SgValue,
};

/// Scratch 3 JSON exporter.
#[derive(Debug, Clone, Default)]
pub struct JsonExporter;

impl JsonExporter {
    pub fn new() -> Self {
        Self
    }
}

impl ScratchExporter for JsonExporter {
    type Output = Value;
    type Error = Infallible;

    fn export(&self, project: &Project) -> Result<Value, Infallible> {
        let mut state = ExportState::new();
        let stage_json = export_stage(&project.stage, &mut state);

        let mut targets = Vec::new();
        targets.push(stage_json);
        for sprite in &project.sprites {
            targets.push(export_sprite(sprite, &mut state));
        }

        Ok(json!({
            "targets": targets,
            "monitors": [],
            "extensions": [],
            "meta": {
                "semver": "3.0.0",
                "vm": "0.2.0",
                "agent": "ScratchArch",
            }
        }))
    }
}

struct ExportState {
    next_id: u64,
    blocks: Map<String, Value>,
}

impl ExportState {
    fn new() -> Self {
        Self {
            next_id: 0,
            blocks: Map::new(),
        }
    }

    fn fresh_id(&mut self) -> String {
        let id = self.next_id;
        self.next_id += 1;
        format!("b{id}")
    }

    fn add_block(&mut self, id: String, block: Value) {
        self.blocks.insert(id, block);
    }
}

fn export_stage(stage: &Stage, state: &mut ExportState) -> Value {
    let mut target = base_target(&stage.name, true);

    let mut variables = Map::new();
    for v in &stage.variables {
        variables.insert(v.id.clone(), json!([v.name, ""]));
    }
    target.insert("variables".to_string(), Value::Object(variables));

    let mut lists = Map::new();
    for l in &stage.lists {
        lists.insert(l.id.clone(), json!([l.name, []]));
    }
    target.insert("lists".to_string(), Value::Object(lists));

    let mut broadcasts = Map::new();
    for b in &stage.broadcasts {
        broadcasts.insert(b.id.clone(), json!(b.name));
    }
    target.insert("broadcasts".to_string(), Value::Object(broadcasts));

    for script in &stage.scripts {
        export_script(script, state);
    }
    for proc in &stage.procedures {
        export_procedure_definition(proc, state);
    }

    target.insert("blocks".to_string(), Value::Object(state.blocks.clone()));
    Value::Object(target)
}

fn export_sprite(sprite: &crate::ir::Sprite, state: &mut ExportState) -> Value {
    let mut target = base_target(&sprite.name, false);

    let mut variables = Map::new();
    for v in &sprite.variables {
        variables.insert(v.id.clone(), json!([v.name, ""]));
    }
    target.insert("variables".to_string(), Value::Object(variables));

    let mut lists = Map::new();
    for l in &sprite.lists {
        lists.insert(l.id.clone(), json!([l.name, []]));
    }
    target.insert("lists".to_string(), Value::Object(lists));

    for script in &sprite.scripts {
        export_script(script, state);
    }
    for proc in &sprite.procedures {
        export_procedure_definition(proc, state);
    }

    target.insert("blocks".to_string(), Value::Object(state.blocks.clone()));
    Value::Object(target)
}

fn base_target(name: &str, is_stage: bool) -> Map<String, Value> {
    let mut target = Map::new();
    target.insert("name".to_string(), json!(name));
    target.insert("isStage".to_string(), json!(is_stage));
    target.insert("x".to_string(), json!(0));
    target.insert("y".to_string(), json!(0));
    target.insert("size".to_string(), json!(100));
    target.insert("direction".to_string(), json!(90));
    target.insert("draggable".to_string(), json!(false));
    target.insert("rotationStyle".to_string(), json!("all around"));
    target.insert("visible".to_string(), json!(true));
    target.insert("costumeIndex".to_string(), json!(0));
    target.insert("costumeCount".to_string(), json!(1));
    target.insert("soundIndex".to_string(), json!(0));
    target.insert("soundCount".to_string(), json!(0));
    target.insert("volume".to_string(), json!(100));
    target.insert("tempo".to_string(), json!(60));
    target.insert("videoTransparency".to_string(), json!(50));
    target.insert("videoState".to_string(), json!("off"));
    target
}

fn export_script(script: &Script, state: &mut ExportState) {
    let hat_id = state.fresh_id();
    let hat_block = match &script.hat {
        Hat::GreenFlag => json!({
            "opcode": "event_whenflagclicked",
            "next": next_field_for_body(&script.body, state),
            "parent": null,
            "inputs": {},
            "fields": {},
            "shadow": false,
            "topLevel": true,
            "x": 0,
            "y": 0,
        }),
        Hat::KeyPressed(key) => json!({
            "opcode": "event_whenkeypressed",
            "next": next_field_for_body(&script.body, state),
            "parent": null,
            "inputs": {},
            "fields": { "KEY_OPTION": [key, null] },
            "shadow": false,
            "topLevel": true,
            "x": 0,
            "y": 0,
        }),
        Hat::SpriteClicked => json!({
            "opcode": "event_whenthisspriteclicked",
            "next": next_field_for_body(&script.body, state),
            "parent": null,
            "inputs": {},
            "fields": {},
            "shadow": false,
            "topLevel": true,
            "x": 0,
            "y": 0,
        }),
        Hat::BroadcastReceived(name) => json!({
            "opcode": "event_whenbroadcastreceived",
            "next": next_field_for_body(&script.body, state),
            "parent": null,
            "inputs": {},
            "fields": { "BROADCAST_OPTION": [name, null] },
            "shadow": false,
            "topLevel": true,
            "x": 0,
            "y": 0,
        }),
        Hat::CloneStart => json!({
            "opcode": "event_whencloned",
            "next": next_field_for_body(&script.body, state),
            "parent": null,
            "inputs": {},
            "fields": {},
            "shadow": false,
            "topLevel": true,
            "x": 0,
            "y": 0,
        }),
        Hat::Procedure { .. } => {
            // Procedure definitions are emitted separately; a script with a
            // procedure hat should not appear as a standalone script.
            unreachable!("procedure hats are handled by export_procedure_definition")
        }
    };
    state.add_block(hat_id.clone(), hat_block);
    emit_body(&hat_id, &script.body, state);
}

fn export_procedure_definition(proc: &Procedure, state: &mut ExportState) {
    let proto_id = state.fresh_id();
    let def_id = state.fresh_id();

    let proccode = proccode(&proc.prototype);
    let mut arg_ids = Map::new();
    let mut arg_names = Vec::new();
    for (idx, param) in proc.prototype.params.iter().enumerate() {
        let arg_id = state.fresh_id();
        arg_ids.insert(format!("{idx}"), json!([arg_id]));
        arg_names.push(json!([param.name.clone()]));
    }

    let prototype_block = json!({
        "opcode": "procedures_prototype",
        "next": null,
        "parent": def_id,
        "inputs": arg_ids,
        "fields": {},
        "shadow": true,
        "topLevel": false,
        "mutation": {
            "tagName": "mutation",
            "children": [],
            "proccode": proccode,
            "argumentids": serde_json::to_string(&arg_names).unwrap(),
            "warp": "false",
        }
    });
    state.add_block(proto_id.clone(), prototype_block);

    let def_block = json!({
        "opcode": "procedures_definition",
        "next": next_field_for_body(&proc.body, state),
        "parent": null,
        "inputs": { "custom_block": [1, proto_id] },
        "fields": {},
        "shadow": false,
        "topLevel": true,
        "x": 0,
        "y": 0,
    });
    state.add_block(def_id.clone(), def_block);
    emit_body(&def_id, &proc.body, state);
}

fn proccode(proto: &crate::ir::ProcedurePrototype) -> String {
    if proto.params.is_empty() {
        proto.name.clone()
    } else {
        let parts: Vec<_> = proto.params.iter().map(|_| "%s").collect();
        format!("{} {}", proto.name, parts.join(" "))
    }
}

fn next_field_for_body(body: &[Stmt], state: &mut ExportState) -> Value {
    if body.is_empty() {
        Value::Null
    } else {
        let first_id = emit_stmt_chain(body, state);
        Value::String(first_id)
    }
}

/// Emit a chain of statements and return the id of the first block.
fn emit_stmt_chain(stmts: &[Stmt], state: &mut ExportState) -> String {
    assert!(!stmts.is_empty(), "empty statement chain");
    let mut ids = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        ids.push(emit_stmt(stmt, state));
    }
    for window in ids.windows(2) {
        let prev = &window[0];
        let next = &window[1];
        if let Some(block) = state.blocks.get_mut(prev) {
            block["next"] = Value::String(next.clone());
        }
        if let Some(block) = state.blocks.get_mut(next) {
            block["parent"] = Value::String(prev.clone());
        }
    }
    ids[0].clone()
}

fn emit_body(parent_id: &str, body: &[Stmt], state: &mut ExportState) {
    if body.is_empty() {
        return;
    }
    let first_id = emit_stmt_chain(body, state);
    if let Some(block) = state.blocks.get_mut(&first_id) {
        block["parent"] = Value::String(parent_id.to_string());
    }
}

fn emit_stmt(stmt: &Stmt, state: &mut ExportState) -> String {
    let id = state.fresh_id();
    let block = match stmt {
        Stmt::Expr(expr) => {
            // Expressions used as statements are typically procedure calls.
            // For other reporter expressions this is a no-op in Scratch; we
            // emit the block anyway so the structure is preserved.
            let (opcode, inputs, fields) = emit_expr_as_statement(expr, state);
            json!({
                "opcode": opcode,
                "next": null,
                "parent": null,
                "inputs": inputs,
                "fields": fields,
                "shadow": false,
                "topLevel": false,
            })
        }
        Stmt::SetVariable { var, value } => json!({
            "opcode": "data_setvariableto",
            "next": null,
            "parent": null,
            "inputs": { "VALUE": emit_input(value, state) },
            "fields": { "VARIABLE": [var, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::ChangeVariable { var, delta } => json!({
            "opcode": "data_changevariableby",
            "next": null,
            "parent": null,
            "inputs": { "VALUE": emit_input(delta, state) },
            "fields": { "VARIABLE": [var, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::AddToList { list, value } => json!({
            "opcode": "data_addtolist",
            "next": null,
            "parent": null,
            "inputs": { "ITEM": emit_input(value, state) },
            "fields": { "LIST": [list, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::DeleteAllOfList { list } => json!({
            "opcode": "data_deletealloflist",
            "next": null,
            "parent": null,
            "inputs": {},
            "fields": { "LIST": [list, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::SetListItem { list, index, value } => json!({
            "opcode": "data_replaceitemoflist",
            "next": null,
            "parent": null,
            "inputs": {
                "INDEX": emit_input(index, state),
                "ITEM": emit_input(value, state),
            },
            "fields": { "LIST": [list, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::DeleteListItem { list, index } => json!({
            "opcode": "data_deleteoflist",
            "next": null,
            "parent": null,
            "inputs": { "INDEX": emit_input(index, state) },
            "fields": { "LIST": [list, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::InsertListItem { list, index, value } => json!({
            "opcode": "data_insertatlist",
            "next": null,
            "parent": null,
            "inputs": {
                "INDEX": emit_input(index, state),
                "ITEM": emit_input(value, state),
            },
            "fields": { "LIST": [list, null] },
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::Broadcast { message } => json!({
            "opcode": "event_broadcast",
            "next": null,
            "parent": null,
            "inputs": { "BROADCAST_INPUT": emit_input(message, state) },
            "fields": {},
            "shadow": false,
            "topLevel": false,
        }),
        Stmt::HeapAlloc { result, size } => {
            // Semantic expansion: result = length of heap before allocation,
            // then repeat `size` times appending 0 to the heap.
            let alloc_stmts = vec![
                Stmt::SetVariable {
                    var: result.clone(),
                    value: Expr::list_length("__scratcharch_heap"),
                },
                Stmt::Repeat {
                    times: size.clone(),
                    body: vec![Stmt::AddToList {
                        list: "__scratcharch_heap".to_string(),
                        value: Expr::number(0.0),
                    }],
                },
            ];
            return emit_stmt_chain(&alloc_stmts, state);
        }
        Stmt::Call { proc, args } => {
            let proccode = if args.is_empty() {
                proc.clone()
            } else {
                format!("{} {}", proc, args.iter().map(|_| "%s").collect::<Vec<_>>().join(" "))
            };
            let mut inputs = Map::new();
            for (idx, arg) in args.iter().enumerate() {
                inputs.insert(format!("{idx}"), emit_input(arg, state));
            }
            json!({
                "opcode": "procedures_call",
                "next": null,
                "parent": null,
                "inputs": inputs,
                "fields": {},
                "shadow": false,
                "topLevel": false,
                "mutation": {
                    "tagName": "mutation",
                    "children": [],
                    "proccode": proccode,
                    "argumentids": serde_json::to_string(&(0..args.len()).map(|i| json!(format!("{i}"))).collect::<Vec<_>>()).unwrap(),
                    "warp": "false",
                }
            })
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let substack1 = if then_body.is_empty() {
                Value::Null
            } else {
                let sub_id = emit_stmt_chain(then_body, state);
                json!([1, sub_id])
            };
            let substack2 = if else_body.is_empty() {
                Value::Null
            } else {
                let sub_id = emit_stmt_chain(else_body, state);
                json!([1, sub_id])
            };
            json!({
                "opcode": "control_if_else",
                "next": null,
                "parent": null,
                "inputs": {
                    "CONDITION": emit_input(condition, state),
                    "SUBSTACK": substack1,
                    "SUBSTACK2": substack2,
                },
                "fields": {},
                "shadow": false,
                "topLevel": false,
            })
        }
        Stmt::Repeat { times, body } => {
            let substack = if body.is_empty() {
                Value::Null
            } else {
                let sub_id = emit_stmt_chain(body, state);
                json!([1, sub_id])
            };
            json!({
                "opcode": "control_repeat",
                "next": null,
                "parent": null,
                "inputs": {
                    "TIMES": emit_input(times, state),
                    "SUBSTACK": substack,
                },
                "fields": {},
                "shadow": false,
                "topLevel": false,
            })
        }
        Stmt::RepeatUntil { condition, body } => {
            let substack = if body.is_empty() {
                Value::Null
            } else {
                let sub_id = emit_stmt_chain(body, state);
                json!([1, sub_id])
            };
            json!({
                "opcode": "control_repeat_until",
                "next": null,
                "parent": null,
                "inputs": {
                    "CONDITION": emit_input(condition, state),
                    "SUBSTACK": substack,
                },
                "fields": {},
                "shadow": false,
                "topLevel": false,
            })
        }
        Stmt::Forever { body } => {
            let substack = if body.is_empty() {
                Value::Null
            } else {
                let sub_id = emit_stmt_chain(body, state);
                json!([1, sub_id])
            };
            json!({
                "opcode": "control_forever",
                "next": null,
                "parent": null,
                "inputs": { "SUBSTACK": substack },
                "fields": {},
                "shadow": false,
                "topLevel": false,
            })
        }
        Stmt::Stop { option } => {
            let option_str = match option {
                StopOption::ThisScript => "this script",
                StopOption::All => "all",
            };
            json!({
                "opcode": "control_stop",
                "next": null,
                "parent": null,
                "inputs": {},
                "fields": { "STOP_OPTION": [option_str, null] },
                "shadow": false,
                "topLevel": false,
            })
        }
    };
    state.add_block(id.clone(), block);
    id
}

fn emit_expr_as_statement(expr: &Expr, state: &mut ExportState) -> (String, Map<String, Value>, Map<String, Value>) {
    match expr {
        Expr::Operator { opcode, args } => {
            let mut inputs = Map::new();
            for (idx, arg) in args.iter().enumerate() {
                inputs.insert(format!("{idx}"), emit_input(arg, state));
            }
            (opcode.clone(), inputs, Map::new())
        }
        _ => ("operator_add".to_string(), Map::new(), Map::new()),
    }
}

fn emit_input(expr: &Expr, state: &mut ExportState) -> Value {
    match expr {
        Expr::Literal(SgValue::Number(n)) => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "math_number",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "NUM": [n.to_string(), null] },
                    "shadow": true,
                    "topLevel": false,
                }),
            );
            json!([1, id])
        }
        Expr::Literal(SgValue::String(s)) => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "text",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "TEXT": [s, null] },
                    "shadow": true,
                    "topLevel": false,
                }),
            );
            json!([1, id])
        }
        Expr::Literal(SgValue::Bool(b)) => {
            // Scratch treats "true"/"false" as booleans in boolean inputs.
            let bool_id = state.fresh_id();
            state.add_block(
                bool_id.clone(),
                json!({
                    "opcode": "text",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "TEXT": [if *b { "true" } else { "false" }, null] },
                    "shadow": true,
                    "topLevel": false,
                }),
            );
            json!([1, bool_id])
        }
        Expr::Variable(name) => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "data_variable",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "VARIABLE": [name, null] },
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::List(name) => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "data_listcontents",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "LIST": [name, null] },
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::ListItem { list, index } => {
            let index_input = emit_input(index, state);
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "data_itemoflist",
                    "next": null,
                    "parent": null,
                    "inputs": {
                        "INDEX": index_input,
                    },
                    "fields": { "LIST": [list, null] },
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::ListLength { list } => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "data_lengthoflist",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "LIST": [list, null] },
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::ProcedureParam(name) => {
            let id = state.fresh_id();
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "argument_reporter_string_number",
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": { "VALUE": [name, null] },
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::HeapLoad { addr } => {
            // Scratch lists are 1-indexed; heap pointers are 0-based offsets.
            let index = Expr::operator(
                "operator_add",
                vec![Expr::number(1.0), (**addr).clone()],
            );
            emit_input(&Expr::ListItem {
                list: "__scratcharch_heap".to_string(),
                index: Box::new(index),
            }, state)
        }
        Expr::HeapIndex { base, offset } => {
            let id = state.fresh_id();
            let mut inputs = Map::new();
            inputs.insert("0".to_string(), emit_input(base, state));
            inputs.insert("1".to_string(), emit_input(offset, state));
            state.add_block(
                id.clone(),
                json!({
                    "opcode": "operator_add",
                    "next": null,
                    "parent": null,
                    "inputs": inputs,
                    "fields": {},
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
        Expr::Operator { opcode, args } => {
            let id = state.fresh_id();
            let mut inputs = Map::new();
            for (idx, arg) in args.iter().enumerate() {
                inputs.insert(format!("{idx}"), emit_input(arg, state));
            }
            state.add_block(
                id.clone(),
                json!({
                    "opcode": opcode,
                    "next": null,
                    "parent": null,
                    "inputs": inputs,
                    "fields": {},
                    "shadow": false,
                    "topLevel": false,
                }),
            );
            json!([2, id])
        }
    }
}
