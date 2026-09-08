//! Corpus member definitions.
//!
//! A *corpus member* is a representative native-Scratch program that the
//! differential corpus commits as a loadable `project.json` fixture (see
//! `tests/corpus.rs`). Members are authored here as IR builders — the same
//! idiom as the roundtrip matrix — so that the committed fixtures are exactly
//! what this toolchain can produce, and regenerating a member is one command.
//!
//! Members deliberately cover the categories the framework is built around:
//! basic data flow, events/broadcasts, procedures, recursion, lists, and
//! native list-as-memory programs. Each member must be *fully valid*: every
//! referenced variable/list is declared, every broadcast that has a receiver
//! is sent, and every procedure is reachable — so graph validation and
//! `verify_project` both pass on the fixture.

use scratcharch_scratchgraph::ir::{
    Broadcast, EventHat, Expr, List, Procedure, ProcedureParam, Project, Script, Stage, Stmt,
    Variable,
};

/// One corpus member: an id (also the fixture file stem), a category, a
/// human-readable description, and the program itself.
pub struct Member {
    pub id: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub project: Project,
}

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn list(name: &str) -> List {
    List::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

fn cat() -> scratcharch_scratchgraph::ir::Sprite {
    scratcharch_scratchgraph::ir::Sprite::new("Cat")
}

fn all_members() -> Vec<Member> {
    vec![
        Member {
            id: "hello_score",
            category: "basic",
            description: "green flag resets a score, repeats an increment, then branches on it",
            project: project_with(hello_score_stage()),
        },
        Member {
            id: "broadcast_relay",
            category: "events",
            description: "stage broadcasts a message; stage and a sprite both receive it",
            project: broadcast_relay_project(),
        },
        Member {
            id: "area_calculator",
            category: "procedures",
            description: "parameterized procedure computes area and is called from a green flag script",
            project: project_with(area_calculator_stage()),
        },
        Member {
            id: "countdown_tick",
            category: "recursion",
            description: "self-recursive procedure with a parameter decremented to a base case",
            project: project_with(countdown_tick_stage()),
        },
        Member {
            id: "shopping_cart",
            category: "lists",
            description: "seed a list, replace and delete items, read item and length back into variables",
            project: project_with(shopping_cart_stage()),
        },
        Member {
            id: "stack_memory",
            category: "memory",
            description: "native list used as a stack: push, peek top by length index, pop",
            project: project_with(stack_memory_stage()),
        },
    ]
}

/// Return the full set of corpus members.
pub fn members() -> Vec<Member> {
    all_members()
}

fn hello_score_stage() -> Stage {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable { var: "score".into(), value: Expr::number(0.0) },
            Stmt::Repeat {
                times: Expr::number(5.0),
                body: vec![Stmt::ChangeVariable {
                    var: "score".into(),
                    delta: Expr::number(1.0),
                }],
            },
            Stmt::If {
                condition: Expr::operator(
                    "operator_gt",
                    vec![Expr::variable("score"), Expr::number(3.0)],
                ),
                then_body: vec![Stmt::SetVariable { var: "score".into(), value: Expr::number(10.0) }],
                else_body: vec![Stmt::SetVariable {
                    var: "score".into(),
                    value: Expr::variable("score"),
                }],
            },
        ],
    ));
    stage
}

fn broadcast_relay_project() -> Project {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("state"));
    stage.add_variable(var("ticks"));
    stage.add_broadcast(Broadcast { id: "go".into(), name: "go".into() });
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable { var: "state".into(), value: Expr::string("idle") },
            Stmt::Broadcast { message: Expr::string("go") },
        ],
    ));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("go".into()),
        vec![Stmt::ChangeVariable { var: "ticks".into(), delta: Expr::number(1.0) }],
    ));

    let mut sprite = cat();
    sprite.add_script(Script::new(
        EventHat::BroadcastReceived("go".into()),
        vec![Stmt::SetVariable { var: "state".into(), value: Expr::string("running") }],
    ));

    let mut project = project_with(stage);
    project.add_sprite(sprite);
    project
}

fn area_calculator_stage() -> Stage {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("area"));
    stage.add_procedure(Procedure::new(
        "rect_area",
        vec![ProcedureParam::new("w"), ProcedureParam::new("h")],
        vec![Stmt::SetVariable {
            var: "area".into(),
            value: Expr::operator(
                "operator_multiply",
                vec![Expr::ProcedureParam("w".into()), Expr::ProcedureParam("h".into())],
            ),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::Call { proc: "rect_area".into(), args: vec![Expr::number(3.0), Expr::number(4.0)] },
            Stmt::SetVariable { var: "area".into(), value: Expr::variable("area") },
        ],
    ));
    stage
}

fn countdown_tick_stage() -> Stage {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("total"));
    stage.add_procedure(Procedure::new(
        "tick",
        vec![ProcedureParam::new("n")],
        vec![
            Stmt::ChangeVariable { var: "total".into(), delta: Expr::ProcedureParam("n".into()) },
            Stmt::If {
                condition: Expr::operator(
                    "operator_gt",
                    vec![Expr::ProcedureParam("n".into()), Expr::number(0.0)],
                ),
                then_body: vec![Stmt::Call {
                    proc: "tick".into(),
                    args: vec![Expr::operator(
                        "operator_subtract",
                        vec![Expr::ProcedureParam("n".into()), Expr::number(1.0)],
                    )],
                }],
                else_body: vec![],
            },
        ],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Call { proc: "tick".into(), args: vec![Expr::number(3.0)] }],
    ));
    stage
}

fn shopping_cart_stage() -> Stage {
    let mut stage = Stage::new("Stage");
    stage.add_list(list("cart"));
    stage.add_variable(var("n"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::DeleteAllOfList { list: "cart".into() },
            Stmt::AddToList { list: "cart".into(), value: Expr::string("apple") },
            Stmt::AddToList { list: "cart".into(), value: Expr::string("banana") },
            Stmt::SetListItem {
                list: "cart".into(),
                index: Expr::number(2.0),
                value: Expr::string("pear"),
            },
            Stmt::DeleteListItem { list: "cart".into(), index: Expr::number(1.0) },
            Stmt::SetVariable { var: "n".into(), value: Expr::list_length("cart") },
            Stmt::SetVariable { var: "n".into(), value: Expr::list_item("cart", Expr::number(1.0)) },
        ],
    ));
    stage
}

fn stack_memory_stage() -> Stage {
    let mut stage = Stage::new("Stage");
    stage.add_list(list("stack"));
    stage.add_variable(var("top"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::DeleteAllOfList { list: "stack".into() },
            Stmt::AddToList { list: "stack".into(), value: Expr::number(1.0) },
            Stmt::AddToList { list: "stack".into(), value: Expr::number(2.0) },
            Stmt::SetVariable {
                var: "top".into(),
                value: Expr::list_item("stack", Expr::list_length("stack")),
            },
            Stmt::DeleteListItem {
                list: "stack".into(),
                index: Expr::list_length("stack"),
            },
            Stmt::SetVariable {
                var: "top".into(),
                value: Expr::operator(
                    "operator_add",
                    vec![
                        Expr::variable("top"),
                        Expr::list_item("stack", Expr::list_length("stack")),
                    ],
                ),
            },
        ],
    ));
    stage
}
