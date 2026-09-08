//! Procedures: custom-block definitions, parameters, and calls.
//!
//! Procedure definitions are per-target and calls bind by name within the
//! owning target (SCRATCH_SEMANTICS.md §2.3). A procedure with parameters
//! round-trips when the call carries one positional argument expression per
//! declared parameter.

use scratcharch_scratchgraph::ir::{
    EventHat, Expr, Procedure, ProcedureParam, Project, Script, Stage, Stmt, Variable,
};

use crate::support::assert_roundtrip_preserves;

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

#[test]
fn define_and_call_a_parameterless_procedure() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_procedure(Procedure::new(
        "bump",
        vec![],
        vec![Stmt::ChangeVariable {
            var: "score".into(),
            delta: Expr::number(1.0),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Call {
            proc: "bump".into(),
            args: vec![],
        }],
    ));
    assert_roundtrip_preserves(
        &project_with(stage),
        "parameterless procedure definition and call",
    );
}

#[test]
fn procedure_with_one_parameter_reads_it_from_the_body() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_procedure(Procedure::new(
        "set_score_to",
        vec![ProcedureParam::new("s")],
        vec![Stmt::SetVariable {
            var: "score".into(),
            value: Expr::ProcedureParam("s".into()),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Call {
            proc: "set_score_to".into(),
            args: vec![Expr::number(7.0)],
        }],
    ));
    assert_roundtrip_preserves(
        &project_with(stage),
        "procedure parameter read inside its body",
    );
}

#[test]
fn procedure_with_two_parameters_and_body_math() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("total"));
    stage.add_procedure(Procedure::new(
        "accumulate",
        vec![ProcedureParam::new("a"), ProcedureParam::new("b")],
        vec![Stmt::SetVariable {
            var: "total".into(),
            value: Expr::operator(
                "operator_add",
                vec![
                    Expr::ProcedureParam("a".into()),
                    Expr::ProcedureParam("b".into()),
                ],
            ),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::Call {
                proc: "accumulate".into(),
                args: vec![Expr::number(2.0), Expr::number(3.0)],
            },
            Stmt::Call {
                proc: "accumulate".into(),
                args: vec![Expr::variable("total"), Expr::number(1.0)],
            },
        ],
    ));
    assert_roundtrip_preserves(
        &project_with(stage),
        "two-parameter procedure with arithmetic body",
    );
}

#[test]
fn calls_nest_inside_control_flow() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_procedure(Procedure::new(
        "penalize",
        vec![],
        vec![Stmt::ChangeVariable {
            var: "score".into(),
            delta: Expr::number(-5.0),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::If {
                condition: Expr::operator(
                    "operator_lt",
                    vec![Expr::variable("score"), Expr::number(0.0)],
                ),
                then_body: vec![Stmt::Call {
                    proc: "penalize".into(),
                    args: vec![],
                }],
                else_body: vec![Stmt::Repeat {
                    times: Expr::number(2.0),
                    body: vec![Stmt::Call {
                        proc: "penalize".into(),
                        args: vec![],
                    }],
                }],
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "procedure call inside if/repeat");
}

#[test]
fn two_procedures_call_each_other() {
    // `a` calls `b`; both are called by scripts. Call graph edges are
    // preserved because calls reference procedures by name.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("x"));
    stage.add_procedure(Procedure::new(
        "a",
        vec![],
        vec![
            Stmt::ChangeVariable {
                var: "x".into(),
                delta: Expr::number(1.0),
            },
            Stmt::Call {
                proc: "b".into(),
                args: vec![],
            },
        ],
    ));
    stage.add_procedure(Procedure::new(
        "b",
        vec![],
        vec![Stmt::ChangeVariable {
            var: "x".into(),
            delta: Expr::number(10.0),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Call {
            proc: "a".into(),
            args: vec![],
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "two procedures call each other");
}
