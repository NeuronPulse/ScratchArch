//! Tests for the basic project.json → ScratchGraph parser.

use serde_json::json;
use scratcharch_scratchgraph::ir::{
    EventHat, Expr, ListScope, ProcedureParam, StopOption, VariableScope,
};
use scratcharch_scratchgraph::parse_project_json;

fn make_minimal_project() -> serde_json::Value {
    json!({
        "targets": [
            {
                "isStage": true,
                "name": "Stage",
                "variables": {
                    "var1": ["my variable", "0"]
                },
                "lists": {
                    "list1": ["my list", []]
                },
                "broadcasts": {
                    "broad1": "hello"
                },
                "blocks": {
                    "hat1": {
                        "opcode": "event_whenflagclicked",
                        "next": "setvar",
                        "parent": null,
                        "topLevel": true,
                        "inputs": {},
                        "fields": {}
                    },
                    "setvar": {
                        "opcode": "data_setvariableto",
                        "next": "stopblock",
                        "parent": "hat1",
                        "inputs": {
                            "VALUE": [1, [10, "10"]]
                        },
                        "fields": {
                            "VARIABLE": ["my variable", "var1"]
                        }
                    },
                    "stopblock": {
                        "opcode": "control_stop",
                        "next": null,
                        "parent": "setvar",
                        "inputs": {},
                        "fields": {
                            "STOP_OPTION": ["all", null]
                        }
                    }
                }
            }
        ]
    })
}

#[test]
fn test_parse_variables_lists_broadcasts() {
    let project = parse_project_json(&make_minimal_project()).unwrap();
    let stage = &project.stage;

    assert_eq!(stage.variables.len(), 1);
    assert_eq!(stage.variables[0].name, "my variable");
    assert!(matches!(stage.variables[0].scope, VariableScope::Global));

    assert_eq!(stage.lists.len(), 1);
    assert_eq!(stage.lists[0].name, "my list");
    assert!(matches!(stage.lists[0].scope, ListScope::Global));

    assert_eq!(stage.broadcasts.len(), 1);
    assert_eq!(stage.broadcasts[0].name, "hello");
}

#[test]
fn test_parse_green_flag_script() {
    let project = parse_project_json(&make_minimal_project()).unwrap();
    let stage = &project.stage;

    assert_eq!(stage.scripts.len(), 1);
    assert!(matches!(&stage.scripts[0].entry.hat, EventHat::GreenFlag));
    assert_eq!(stage.scripts[0].entry.body.len(), 2);
}

#[test]
fn test_parse_set_variable_and_stop() {
    let project = parse_project_json(&make_minimal_project()).unwrap();
    let body = &project.stage.scripts[0].entry.body;

    assert!(matches!(body[0], scratcharch_scratchgraph::ir::Stmt::SetVariable { .. }));
    assert!(matches!(body[1], scratcharch_scratchgraph::ir::Stmt::Stop { option: StopOption::All }));
}

#[test]
fn test_parse_procedure() {
    let value = json!({
        "targets": [
            {
                "isStage": true,
                "name": "Stage",
                "variables": {},
                "lists": {},
                "broadcasts": {},
                "blocks": {
                    "def": {
                        "opcode": "procedures_definition",
                        "next": null,
                        "parent": null,
                        "topLevel": true,
                        "mutation": {
                            "proccode": "add %s %s",
                            "argumentnames": "[\"a\", \"b\"]",
                            "argumentids": "[\"arg0\", \"arg1\"]"
                        }
                    }
                }
            }
        ]
    });

    let project = parse_project_json(&value).unwrap();
    assert_eq!(project.stage.procedures.len(), 1);
    let proc = &project.stage.procedures[0];
    assert_eq!(proc.prototype.name, "add");
    assert_eq!(proc.prototype.params.len(), 2);
    assert_eq!(proc.prototype.params[0], ProcedureParam { name: "a".to_string(), default: None });
}

#[test]
fn test_parse_arithmetic_expression() {
    let value = json!({
        "targets": [
            {
                "isStage": true,
                "name": "Stage",
                "variables": {},
                "lists": {},
                "broadcasts": {},
                "blocks": {
                    "hat": {
                        "opcode": "event_whenflagclicked",
                        "next": "set",
                        "parent": null,
                        "topLevel": true,
                        "inputs": {},
                        "fields": {}
                    },
                    "set": {
                        "opcode": "data_setvariableto",
                        "next": null,
                        "parent": "hat",
                        "inputs": {
                            "VALUE": [1, "addid"]
                        },
                        "fields": {
                            "VARIABLE": ["x", "var1"]
                        }
                    },
                    "addid": {
                        "opcode": "operator_add",
                        "next": null,
                        "parent": "set",
                        "inputs": {
                            "NUM1": [1, [10, "2"]],
                            "NUM2": [1, [10, "3"]]
                        },
                        "fields": {}
                    }
                }
            }
        ]
    });

    let project = parse_project_json(&value).unwrap();
    let body = &project.stage.scripts[0].entry.body;
    assert!(matches!(
        body[0],
        scratcharch_scratchgraph::ir::Stmt::SetVariable {
            value: Expr::Operator { ref opcode, .. },
            ..
        } if opcode == "operator_add"
    ));
}

#[test]
fn test_parse_if_block() {
    let value = json!({
        "targets": [
            {
                "isStage": true,
                "name": "Stage",
                "variables": {},
                "lists": {},
                "broadcasts": {},
                "blocks": {
                    "hat": {
                        "opcode": "event_whenflagclicked",
                        "next": "if",
                        "parent": null,
                        "topLevel": true,
                        "inputs": {},
                        "fields": {}
                    },
                    "if": {
                        "opcode": "control_if",
                        "next": null,
                        "parent": "hat",
                        "inputs": {
                            "CONDITION": [1, [4, ""]],
                            "SUBSTACK": [2, "inner"]
                        },
                        "fields": {}
                    },
                    "inner": {
                        "opcode": "control_stop",
                        "next": null,
                        "parent": "if",
                        "inputs": {},
                        "fields": {
                            "STOP_OPTION": ["this script", null]
                        }
                    }
                }
            }
        ]
    });

    let project = parse_project_json(&value).unwrap();
    let body = &project.stage.scripts[0].entry.body;
    assert_eq!(body.len(), 1);
    assert!(matches!(body[0], scratcharch_scratchgraph::ir::Stmt::If { ref then_body, .. } if then_body.len() == 1));
}
