//! Basic programs: event hats, plain data flow, structured control.
//!
//! These programs stay entirely inside the native-Scratch roundtrippable
//! subset (see `docs/specification/SCRATCH_SEMANTICS.md` §5): variables and
//! lists are stage-declared and global, statements are `data_*` /
//! `event_broadcast` / control blocks, and expression operators come from the
//! arithmetic/comparison set the parser reconstructs.

use scratcharch_scratchgraph::ir::{
    EventHat, Expr, Project, Script, Stage, StopOption, Stmt, Variable,
};

use crate::support::assert_roundtrip_preserves;

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

#[test]
fn green_flag_sets_a_constant() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::SetVariable {
            var: "score".into(),
            value: Expr::number(10.0),
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "green flag sets a constant");
}

#[test]
fn key_press_changes_a_variable() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_script(Script::new(
        EventHat::KeyPressed("space".into()),
        vec![Stmt::ChangeVariable {
            var: "score".into(),
            delta: Expr::number(1.0),
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "space key changes a variable");
}

#[test]
fn sprite_click_increments_a_global_variable() {
    // The variable is stage-declared (global); the incrementing script lives
    // on a sprite. Scripts may read/write globals from any target.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    let mut sprite = scratcharch_scratchgraph::ir::Sprite::new("Cat");
    sprite.add_script(Script::new(
        EventHat::SpriteClicked,
        vec![Stmt::ChangeVariable {
            var: "score".into(),
            delta: Expr::number(1.0),
        }],
    ));
    let mut project = project_with(stage);
    project.add_sprite(sprite);
    assert_roundtrip_preserves(&project, "sprite click increments a global");
}

#[test]
fn clone_start_runs_its_body() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("clones"));
    stage.add_script(Script::new(
        EventHat::CloneStart,
        vec![Stmt::ChangeVariable {
            var: "clones".into(),
            delta: Expr::number(1.0),
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "clone start body");
}

#[test]
fn if_then_else_picks_a_branch() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::If {
            condition: Expr::operator(
                "operator_gt",
                vec![Expr::variable("score"), Expr::number(0.0)],
            ),
            then_body: vec![Stmt::SetVariable {
                var: "result".into(),
                value: Expr::string("positive"),
            }],
            else_body: vec![Stmt::SetVariable {
                var: "result".into(),
                value: Expr::string("not positive"),
            }],
        }],
    ));
    // Note: `result` is only referenced, never declared; undeclared references
    // survive the roundtrip as names and are reported consistently.
    assert_roundtrip_preserves(&project_with(stage), "if/else picks a branch");
}

#[test]
fn repeat_runs_its_body_three_times() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("count"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "count".into(),
                value: Expr::number(0.0),
            },
            Stmt::Repeat {
                times: Expr::number(3.0),
                body: vec![Stmt::ChangeVariable {
                    var: "count".into(),
                    delta: Expr::number(1.0),
                }],
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "repeat runs three times");
}

#[test]
fn repeat_until_loops_until_condition() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("count"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::RepeatUntil {
            condition: Expr::operator(
                "operator_gt",
                vec![Expr::variable("count"), Expr::number(5.0)],
            ),
            body: vec![Stmt::ChangeVariable {
                var: "count".into(),
                delta: Expr::number(1.0),
            }],
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "repeat until loops");
}

#[test]
fn stop_all_terminates_everything() {
    let mut stage = Stage::new("Stage");
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "x".into(),
                value: Expr::number(1.0),
            },
            Stmt::Stop {
                option: StopOption::All,
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "stop all");
}

#[test]
fn stop_this_script_is_distinct_from_stop_all() {
    let mut stage = Stage::new("Stage");
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Stop {
            option: StopOption::ThisScript,
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "stop this script");
}

#[test]
fn nested_control_flow_roundtrips() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("score"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Repeat {
            times: Expr::number(4.0),
            body: vec![Stmt::If {
                condition: Expr::operator(
                    "operator_equals",
                    vec![Expr::variable("score"), Expr::number(0.0)],
                ),
                then_body: vec![Stmt::ChangeVariable {
                    var: "score".into(),
                    delta: Expr::number(1.0),
                }],
                else_body: vec![Stmt::SetVariable {
                    var: "score".into(),
                    value: Expr::operator(
                        "operator_multiply",
                        vec![Expr::variable("score"), Expr::number(2.0)],
                    ),
                }],
            }],
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "repeat containing an if/else");
}

#[test]
fn arithmetic_expression_tree_roundtrips() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("v"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::SetVariable {
            var: "v".into(),
            value: Expr::operator(
                "operator_subtract",
                vec![
                    Expr::operator(
                        "operator_add",
                        vec![Expr::number(2.0), Expr::number(3.0)],
                    ),
                    Expr::operator(
                        "operator_divide",
                        vec![Expr::number(10.0), Expr::number(2.0)],
                    ),
                ],
            ),
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "arithmetic expression tree");
}
