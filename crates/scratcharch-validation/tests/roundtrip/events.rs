//! Events: broadcasts and their receivers, including across targets.
//!
//! Broadcast messages are project-global (SCRATCH_SEMANTICS.md §2.2): a
//! `broadcast` on any target can wake a `BroadcastReceived` hat on any other,
//! and the declaration site of a message is not semantic. These cases assert
//! that send/receive pairs survive the roundtrip unchanged.

use scratcharch_scratchgraph::ir::{
    Broadcast, EventHat, Expr, Project, Script, Sprite, Stage, Stmt, Variable,
};

use crate::support::assert_roundtrip_preserves;

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

#[test]
fn broadcast_between_two_scripts_on_the_stage() {
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b1".into(), name: "start".into() });
    stage.add_variable(var("started"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Broadcast {
            message: Expr::string("start"),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("start".into()),
        vec![Stmt::SetVariable {
            var: "started".into(),
            value: Expr::number(1.0),
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "broadcast/receive on the stage");
}

#[test]
fn broadcast_across_targets_reaches_a_sprite() {
    // The stage broadcasts; a sprite holds the receiver and writes a
    // stage-declared global when it wakes.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-go".into(), name: "go".into() });
    stage.add_variable(var("state"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "state".into(),
                value: Expr::string("ready"),
            },
            Stmt::Broadcast {
                message: Expr::string("go"),
            },
        ],
    ));

    let mut pong = Sprite::new("Pong");
    pong.add_script(Script::new(
        EventHat::BroadcastReceived("go".into()),
        vec![Stmt::SetVariable {
            var: "state".into(),
            value: Expr::string("running"),
        }],
    ));

    let mut project = project_with(stage);
    project.add_sprite(pong);
    assert_roundtrip_preserves(&project, "stage broadcast reaches a sprite receiver");
}

#[test]
fn a_sprite_broadcasts_to_the_stage() {
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-done".into(), name: "done".into() });
    stage.add_variable(var("state"));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("done".into()),
        vec![Stmt::SetVariable {
            var: "state".into(),
            value: Expr::string("finished"),
        }],
    ));

    let mut cat = Sprite::new("Cat");
    cat.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Broadcast {
            message: Expr::string("done"),
        }],
    ));

    let mut project = project_with(stage);
    project.add_sprite(cat);
    assert_roundtrip_preserves(&project, "sprite broadcast reaches a stage receiver");
}

#[test]
fn two_receivers_on_different_targets_survive() {
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-ping".into(), name: "ping".into() });
    stage.add_variable(var("a"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![Stmt::Broadcast {
            message: Expr::string("ping"),
        }],
    ));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("ping".into()),
        vec![Stmt::ChangeVariable {
            var: "a".into(),
            delta: Expr::number(1.0),
        }],
    ));

    let mut b = Sprite::new("B");
    b.add_script(Script::new(
        EventHat::BroadcastReceived("ping".into()),
        vec![Stmt::ChangeVariable {
            var: "a".into(),
            delta: Expr::number(1.0),
        }],
    ));

    let mut project = project_with(stage);
    project.add_sprite(b);
    assert_roundtrip_preserves(&project, "two receivers on different targets");
}

#[test]
fn dynamic_broadcast_message_variable() {
    // A broadcast whose message is read from a variable: the expression
    // (not a literal) is preserved, keeping every receiver potentially live.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-any".into(), name: "any".into() });
    stage.add_variable(var("msg"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::SetVariable {
                var: "msg".into(),
                value: Expr::string("any"),
            },
            Stmt::Broadcast {
                message: Expr::variable("msg"),
            },
        ],
    ));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("any".into()),
        vec![Stmt::Stop {
            option: scratcharch_scratchgraph::ir::StopOption::ThisScript,
        }],
    ));
    assert_roundtrip_preserves(&project_with(stage), "broadcast message read from a variable");
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}
