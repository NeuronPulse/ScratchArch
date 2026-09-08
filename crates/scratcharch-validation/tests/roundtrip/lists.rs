//! Lists: list mutation statements and list-read expressions.
//!
//! Lists are declared per target with `(name, scope)` identity like variables
//! (SCRATCH_SEMANTICS.md §2.1). The statements exercised here are the `data_*`
//! list operations, and the expressions are `item … of list` /
//! `length of list` reads plus their index arithmetic.

use scratcharch_scratchgraph::ir::{
    EventHat, Expr, List, Project, Script, Stage, Stmt, Variable,
};

use crate::support::assert_roundtrip_preserves;

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn list(name: &str) -> List {
    List::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

#[test]
fn seed_clear_and_read_items() {
    let mut stage = Stage::new("Stage");
    stage.add_list(list("notes"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::DeleteAllOfList {
                list: "notes".into(),
            },
            Stmt::AddToList {
                list: "notes".into(),
                value: Expr::string("first"),
            },
            Stmt::AddToList {
                list: "notes".into(),
                value: Expr::string("second"),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "delete-all then append two items");
}

#[test]
fn item_and_length_reads_roundtrip() {
    let mut stage = Stage::new("Stage");
    stage.add_list(list("notes"));
    stage.add_variable(var("cursor"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::AddToList {
                list: "notes".into(),
                value: Expr::number(41.0),
            },
            Stmt::SetVariable {
                var: "cursor".into(),
                value: Expr::list_length("notes"),
            },
            Stmt::ChangeVariable {
                var: "cursor".into(),
                delta: Expr::number(1.0),
            },
            Stmt::SetVariable {
                var: "cursor".into(),
                value: Expr::list_item("notes", Expr::number(1.0)),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "item and length of list reads");
}

#[test]
fn insert_replace_and_delete_items() {
    let mut stage = Stage::new("Stage");
    stage.add_list(list("notes"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::AddToList {
                list: "notes".into(),
                value: Expr::string("x"),
            },
            Stmt::AddToList {
                list: "notes".into(),
                value: Expr::string("y"),
            },
            Stmt::SetListItem {
                list: "notes".into(),
                index: Expr::number(2.0),
                value: Expr::string("replaced"),
            },
            Stmt::InsertListItem {
                list: "notes".into(),
                index: Expr::number(1.0),
                value: Expr::string("inserted"),
            },
            Stmt::DeleteListItem {
                list: "notes".into(),
                index: Expr::number(3.0),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "replace, insert and delete list items");
}

#[test]
fn list_accumulator_pattern() {
    // A counter driven by list length, the shape used for stacks.
    let mut stage = Stage::new("Stage");
    stage.add_list(list("stack"));
    stage.add_variable(var("top"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::AddToList {
                list: "stack".into(),
                value: Expr::number(7.0),
            },
            Stmt::SetVariable {
                var: "top".into(),
                value: Expr::list_length("stack"),
            },
            Stmt::SetVariable {
                var: "top".into(),
                value: Expr::list_item(
                    "stack",
                    Expr::operator(
                        "operator_add",
                        vec![Expr::list_length("stack"), Expr::number(-1.0)],
                    ),
                ),
            },
            Stmt::DeleteListItem {
                list: "stack".into(),
                index: Expr::list_length("stack"),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "list-as-stack accumulator");
}
