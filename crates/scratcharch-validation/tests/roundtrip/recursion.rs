//! Recursion: procedures whose bodies call themselves (or each other).
//!
//! The roundtrip only needs to preserve the *call structure*: a self-call
//! inside the procedure's own body must come back as a `Call` to the same
//! name. (Nothing here executes the recursion; that is the interpreter's job.)

use scratcharch_scratchgraph::ir::{
    EventHat, Expr, Procedure, Project, Script, Stage, Stmt, Variable,
};

use crate::support::assert_roundtrip_preserves;

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

#[test]
fn direct_recursion_calls_itself() {
    // countdown(n): while count > 0, decrement and call countdown again.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("count"));
    stage.add_procedure(Procedure::new(
        "countdown",
        vec![],
        vec![Stmt::If {
            condition: Expr::operator(
                "operator_gt",
                vec![Expr::variable("count"), Expr::number(0.0)],
            ),
            then_body: vec![
                Stmt::ChangeVariable {
                    var: "count".into(),
                    delta: Expr::number(-1.0),
                },
                Stmt::Call {
                    proc: "countdown".into(),
                    args: vec![],
                },
            ],
            else_body: vec![],
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "count".into(),
                value: Expr::number(3.0),
            },
            Stmt::Call {
                proc: "countdown".into(),
                args: vec![],
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "direct recursion");
}

#[test]
fn recursive_procedure_with_parameter() {
    // fib-ish: set acc = acc + n; if n > 1 recurse with n - 1.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("acc"));
    stage.add_procedure(Procedure::new(
        "add_down",
        vec![scratcharch_scratchgraph::ir::ProcedureParam::new("n")],
        vec![
            Stmt::ChangeVariable {
                var: "acc".into(),
                delta: Expr::ProcedureParam("n".into()),
            },
            Stmt::If {
                condition: Expr::operator(
                    "operator_gt",
                    vec![Expr::ProcedureParam("n".into()), Expr::number(1.0)],
                ),
                then_body: vec![Stmt::Call {
                    proc: "add_down".into(),
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
        vec![Stmt::Call {
            proc: "add_down".into(),
            args: vec![Expr::number(4.0)],
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "recursive procedure with parameter");
}

#[test]
fn mutual_recursion_across_two_procedures() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("parity"));
    stage.add_procedure(Procedure::new(
        "flip_to",
        vec![],
        vec![
            Stmt::ChangeVariable {
                var: "parity".into(),
                delta: Expr::number(1.0),
            },
            Stmt::If {
                condition: Expr::operator(
                    "operator_lt",
                    vec![Expr::variable("parity"), Expr::number(3.0)],
                ),
                then_body: vec![Stmt::Call {
                    proc: "flip_back".into(),
                    args: vec![],
                }],
                else_body: vec![],
            },
        ],
    ));
    stage.add_procedure(Procedure::new(
        "flip_back",
        vec![],
        vec![
            Stmt::ChangeVariable {
                var: "parity".into(),
                delta: Expr::number(-1.0),
            },
            Stmt::Call {
                proc: "flip_to".into(),
                args: vec![],
            },
        ],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Call {
            proc: "flip_to".into(),
            args: vec![],
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "mutual recursion");
}
