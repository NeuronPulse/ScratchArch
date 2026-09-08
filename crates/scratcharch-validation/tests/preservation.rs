//! Transform-preservation tests.
//!
//! For each ScratchGraph pass the framework computes
//! `Before → Normalize → Transform → Normalize → Semantic Diff`, and the
//! verdicts in `scratcharch_validation::preservation` assert that no
//! *disallowed* change occurred (they are written independently of the pass
//! implementations). These tests pin down the spec rules:
//!
//! - Dead-script elimination is forbidden from removing a reachable event
//!   receiver, a live procedure, or from changing broadcast semantics
//!   (SCRATCH_SEMANTICS.md §4). It may remove only provably-dead receivers,
//!   and the framework must *report* that removal, never bless it silently.
//! - Constant folding is forbidden from changing runtime behavior: it may
//!   replace constant numeric arithmetic with its IEEE-754 value and nothing
//!   else. Comparisons and division by zero are never folded, so a boolean
//!   context can never collapse into a numeric literal.
//! - Variable analysis may drop only declarations nothing references.
//! - Empty-block removal may drop only empty control wrappers.
//!
//! Each pass therefore has two kinds of test:
//!   * **no-change guards** on fully-live programs — the pass must leave the
//!     semantic diff empty, and
//!   * **purpose tests** where the pass legitimately transforms dead/foldable
//!     code — the framework verdict stays OK (the pass did exactly what it is
//!     allowed to) while the semantic diff honestly reports the change.

use scratcharch_analyzer::{semantic_diff, DiffFormat};
use scratcharch_scratchgraph::ir::{
    Broadcast, EventHat, Expr, Procedure, Project, Script, Sprite, Stage, Stmt, Variable,
};
use scratcharch_transform::{
    ConstantFolding, DeadScriptElimination, EmptyBlockRemoval, TransformPass, VariableAnalysis,
};
use scratcharch_validation::{
    constant_folding_reaches_canonical_fold, dce_preserves_liveness,
    empty_block_removal_matches, variable_analysis_keeps_referenced,
};

// ---------------------------------------------------------------- helpers

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn num(v: f64) -> Expr {
    Expr::number(v)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

fn project_with_sprite(stage: Stage, sprite: Sprite) -> Project {
    let mut p = project_with(stage);
    p.add_sprite(sprite);
    p
}

/// Apply one pass to a clone and return (result, report).
fn apply(
    pass: &mut dyn TransformPass,
    project: &Project,
) -> (Project, scratcharch_transform::PassReport) {
    let mut out = project.clone();
    let report = pass.run(&mut out);
    (out, report)
}

fn assert_semantically_equal(a: &Project, b: &Project, what: &str) {
    let diff = semantic_diff(a, b);
    assert!(
        diff.is_empty(),
        "{what}: unexpected semantic difference:\n{}",
        diff.format(DiffFormat::Text)
    );
}

fn assert_semantically_different(a: &Project, b: &Project, what: &str) {
    let diff = semantic_diff(a, b);
    assert!(
        !diff.is_empty(),
        "{what}: expected the framework to report a semantic change, but it blessed the pass"
    );
}

fn green_flag(body: Vec<Stmt>) -> Script {
    Script::new(EventHat::GreenFlag, body)
}

// ================================================================ DCE

#[test]
fn dce_keeps_a_receiver_broadcast_from_another_target() {
    // The stage broadcasts "go"; a sprite wakes on it. Both are reachable, so
    // DCE must remove nothing.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-go".into(), name: "go".into() });
    stage.add_variable(var("state"));
    stage.add_script(green_flag(vec![Stmt::Broadcast { message: Expr::string("go") }]));
    let mut pong = Sprite::new("Pong");
    pong.add_script(Script::new(
        EventHat::BroadcastReceived("go".into()),
        vec![Stmt::SetVariable { var: "state".into(), value: num(1.0) }],
    ));
    let project = project_with_sprite(stage, pong);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "nothing is dead in this project");
    dce_preserves_liveness(&project, &after).expect("reachable receiver must survive DCE");
    assert_semantically_equal(&project, &after, "DCE on a fully-live project");
}

#[test]
fn dce_keeps_a_receiver_when_the_send_lives_in_a_procedure() {
    // "emit" broadcasts from inside a procedure body; that broadcast still
    // reaches the sprite receiver, so procedure and receiver must both survive.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-go".into(), name: "go".into() });
    stage.add_variable(var("state"));
    stage.add_procedure(Procedure::new(
        "emit",
        vec![],
        vec![Stmt::Broadcast { message: Expr::string("go") }],
    ));
    stage.add_script(green_flag(vec![Stmt::Call { proc: "emit".into(), args: vec![] }]));

    let mut recv = Sprite::new("Recv");
    recv.add_script(Script::new(
        EventHat::BroadcastReceived("go".into()),
        vec![Stmt::SetVariable { var: "state".into(), value: num(1.0) }],
    ));
    let project = project_with_sprite(stage, recv);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "the broadcasting procedure is live");
    dce_preserves_liveness(&project, &after)
        .expect("procedure-sent broadcast must keep its receiver");
    assert_semantically_equal(&project, &after, "DCE must not drop the emitter/receiver pair");
}

#[test]
fn dce_keeps_a_procedure_called_only_from_a_receiver() {
    // "react" is reachable only from the sprite receiver's body, but the
    // receiver itself is live, so the whole call chain must survive.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-start".into(), name: "start".into() });
    stage.add_variable(var("state"));
    stage.add_script(green_flag(vec![Stmt::Broadcast { message: Expr::string("start") }]));

    let mut spr = Sprite::new("Spr");
    spr.add_procedure(Procedure::new(
        "react",
        vec![],
        vec![Stmt::SetVariable { var: "state".into(), value: num(42.0) }],
    ));
    spr.add_script(Script::new(
        EventHat::BroadcastReceived("start".into()),
        vec![Stmt::Call { proc: "react".into(), args: vec![] }],
    ));
    let project = project_with_sprite(stage, spr);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "receiver and its callee are both live");
    dce_preserves_liveness(&project, &after)
        .expect("a procedure called from a live receiver is itself live");
    let spr_after = after.sprites.iter().find(|s| s.name == "Spr").expect("sprite gone");
    assert!(
        spr_after.procedures.iter().any(|p| p.prototype.name == "react"),
        "live procedure `react` must survive DCE"
    );
    assert_semantically_equal(&project, &after, "DCE on a fully-live project");
}

#[test]
fn dce_keeps_receivers_under_a_dynamic_broadcast() {
    // The broadcast message is read from a variable, so no receiver can be
    // proven unreachable — DCE must keep every `BroadcastReceived` hat.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-foo".into(), name: "foo".into() });
    stage.add_broadcast(Broadcast { id: "b-bar".into(), name: "bar".into() });
    stage.add_variable(var("msg"));
    stage.add_script(green_flag(vec![Stmt::Broadcast { message: Expr::variable("msg") }]));
    stage.add_script(Script::new(EventHat::BroadcastReceived("foo".into()), vec![]));

    let mut spr = Sprite::new("Spr");
    spr.add_script(Script::new(EventHat::BroadcastReceived("bar".into()), vec![]));
    let project = project_with_sprite(stage, spr);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "dynamic broadcast keeps all receivers");
    dce_preserves_liveness(&project, &after)
        .expect("receivers of a dynamic broadcast must survive DCE");
    assert_semantically_equal(&project, &after, "DCE on a dynamically-broadcast project");
}

#[test]
fn dce_keeps_a_self_recursive_procedure() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("count"));
    stage.add_procedure(Procedure::new(
        "countdown",
        vec![],
        vec![Stmt::If {
            condition: Expr::operator("operator_gt", vec![Expr::variable("count"), num(0.0)]),
            then_body: vec![
                Stmt::ChangeVariable { var: "count".into(), delta: num(-1.0) },
                Stmt::Call { proc: "countdown".into(), args: vec![] },
            ],
            else_body: vec![],
        }],
    ));
    stage.add_script(green_flag(vec![Stmt::Call { proc: "countdown".into(), args: vec![] }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "recursive live procedure must be kept");
    dce_preserves_liveness(&project, &after)
        .expect("self-recursive live procedure must survive DCE");
    assert_semantically_equal(&project, &after, "DCE on a recursive live project");
}

#[test]
fn dce_may_remove_a_provably_dead_receiver_but_must_report_it() {
    // "start" is broadcast, so its receiver is live; "never" is never sent, so
    // its receiver is provably dead and DCE removes it. The framework verdict
    // stays OK (only dead code went away) *and* the semantic diff reports the
    // removal — the checker never blesses a lossy conversion silently.
    let mut stage = Stage::new("Stage");
    stage.add_broadcast(Broadcast { id: "b-start".into(), name: "start".into() });
    stage.add_broadcast(Broadcast { id: "b-never".into(), name: "never".into() });
    stage.add_variable(var("x"));
    stage.add_script(green_flag(vec![Stmt::Broadcast { message: Expr::string("start") }]));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("start".into()),
        vec![Stmt::SetVariable { var: "x".into(), value: num(1.0) }],
    ));
    stage.add_script(Script::new(
        EventHat::BroadcastReceived("never".into()),
        vec![Stmt::SetVariable { var: "x".into(), value: num(2.0) }],
    ));
    let project = project_with(stage);

    let (after, report) = apply(&mut DeadScriptElimination::new(), &project);
    assert_eq!(report.deleted_nodes, 1, "exactly the unreachable receiver is dead");
    dce_preserves_liveness(&project, &after)
        .expect("removing a provably-dead receiver is allowed");

    let receivers_of = |p: &Project, msg: &str| {
        p.stage
            .scripts
            .iter()
            .filter(|s| matches!(&s.entry.hat, EventHat::BroadcastReceived(m) if m == msg))
            .count()
    };
    assert_eq!(receivers_of(&after, "never"), 0, "dead receiver removed");
    assert_eq!(receivers_of(&after, "start"), 1, "live receiver kept");
    assert_semantically_different(&project, &after, "DCE removing a dead receiver");
}

// ============================================================== folding

#[test]
fn folding_leaves_programs_without_constant_math_untouched() {
    // Arithmetic over variables is not foldable; comparisons produce Scratch
    // booleans and are never folded even with literal operands.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("a"));
    stage.add_script(green_flag(vec![
        Stmt::SetVariable {
            var: "x".into(),
            value: Expr::operator("operator_add", vec![Expr::variable("a"), num(2.0)]),
        },
        Stmt::If {
            condition: Expr::operator("operator_gt", vec![num(3.0), num(1.0)]),
            then_body: vec![Stmt::SetVariable { var: "y".into(), value: num(1.0) }],
            else_body: vec![],
        },
    ]));
    let project = project_with(stage);

    let (after, report) = apply(&mut ConstantFolding::new(), &project);
    assert_eq!(report.modifications, 0, "nothing here is foldable");
    constant_folding_reaches_canonical_fold(&project, &after)
        .expect("no fold means trivially canonical");
    assert_semantically_equal(&project, &after, "folding on a non-foldable program");
}

#[test]
fn folding_never_collapses_a_boolean_comparison_into_a_number() {
    // A comparison with all-literal operands must stay a comparison: folding it
    // to the numeric literal 1/0 would change Scratch boolean semantics.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("t"));
    stage.add_script(green_flag(vec![
        Stmt::If {
            condition: Expr::operator("operator_equals", vec![num(4.0), num(4.0)]),
            then_body: vec![Stmt::SetVariable { var: "t".into(), value: num(1.0) }],
            else_body: vec![],
        },
        Stmt::SetVariable {
            var: "b".into(),
            value: Expr::operator("operator_gt", vec![num(1.0), num(0.0)]),
        },
    ]));
    let project = project_with(stage);

    let (after, report) = apply(&mut ConstantFolding::new(), &project);
    assert_eq!(report.modifications, 0, "comparisons are never folded");
    constant_folding_reaches_canonical_fold(&project, &after)
        .expect("boolean contexts must be left intact");
    assert_semantically_equal(&project, &after, "folding must not touch comparisons");
}

#[test]
fn folding_leaves_division_by_zero_as_an_expression() {
    // `5 / 0` is runtime NaN in Scratch; folding it would change runtime
    // behavior, so the expression must survive.
    let mut stage = Stage::new("Stage");
    stage.add_script(green_flag(vec![Stmt::SetVariable {
        var: "q".into(),
        value: Expr::operator("operator_divide", vec![num(5.0), num(0.0)]),
    }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut ConstantFolding::new(), &project);
    assert_eq!(report.modifications, 0, "division by zero is never folded");
    constant_folding_reaches_canonical_fold(&project, &after)
        .expect("division-by-zero must be preserved as runtime behavior");
    assert_semantically_equal(&project, &after, "folding of division by zero");
}

#[test]
fn folding_replaces_constant_arithmetic_and_reports_the_change() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("t"));
    stage.add_script(green_flag(vec![
        Stmt::SetVariable {
            var: "x".into(),
            value: Expr::operator("operator_add", vec![num(2.0), num(3.0)]),
        },
        Stmt::ChangeVariable {
            var: "t".into(),
            delta: Expr::operator("operator_multiply", vec![num(2.0), num(4.0)]),
        },
        Stmt::If {
            condition: Expr::operator("operator_equals", vec![num(4.0), num(4.0)]),
            then_body: vec![Stmt::SetVariable { var: "t".into(), value: num(1.0) }],
            else_body: vec![],
        },
    ]));
    let project = project_with(stage);

    let (after, report) = apply(&mut ConstantFolding::new(), &project);
    assert_eq!(report.modifications, 2, "two statements held foldable math");
    constant_folding_reaches_canonical_fold(&project, &after)
        .expect("folding matched the independent IEEE-754 evaluation");
    match &after.stage.scripts[0].entry.body[0] {
        Stmt::SetVariable { value, .. } => assert_eq!(value, &num(5.0), "2+3 must fold to 5"),
        other => panic!("expected SetVariable, got {other:?}"),
    }
    // The comparison condition in statement index 2 must still be a comparison.
    match &after.stage.scripts[0].entry.body[2] {
        Stmt::If { condition, .. } => assert!(matches!(
            condition,
            Expr::Operator { opcode, .. } if opcode == "operator_equals"
        )),
        other => panic!("expected If, got {other:?}"),
    }
    assert_semantically_different(&project, &after, "folding replaced constant math");
}

#[test]
fn folding_nested_arithmetic_reaches_a_fixed_point() {
    // 1 + (2 * 3): the inner multiply folds first, then the outer add.
    let mut stage = Stage::new("Stage");
    stage.add_script(green_flag(vec![Stmt::SetVariable {
        var: "x".into(),
        value: Expr::operator(
            "operator_add",
            vec![num(1.0), Expr::operator("operator_multiply", vec![num(2.0), num(3.0)])],
        ),
    }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut ConstantFolding::new(), &project);
    assert_eq!(report.modifications, 1);
    constant_folding_reaches_canonical_fold(&project, &after)
        .expect("nested folding must reach the same fixed point as independent evaluation");
    match &after.stage.scripts[0].entry.body[0] {
        Stmt::SetVariable { value, .. } => assert_eq!(value, &num(7.0), "1 + (2*3) must fold to 7"),
        other => panic!("expected SetVariable, got {other:?}"),
    }
    assert_semantically_different(&project, &after, "folding replaced nested constant math");
}

// ===================================================== variable analysis

#[test]
fn variable_analysis_keeps_all_referenced_variables() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("g"));
    stage.add_script(green_flag(vec![Stmt::ChangeVariable { var: "g".into(), delta: num(1.0) }]));

    let mut ball = Sprite::new("Ball");
    ball.add_variable(var("local"));
    ball.add_script(green_flag(vec![Stmt::SetVariable {
        var: "local".into(),
        value: Expr::variable("g"),
    }]));
    let project = project_with_sprite(stage, ball);

    let (after, report) = apply(&mut VariableAnalysis::new(), &project);
    assert_eq!(report.saved_variables, 0, "every variable is referenced");
    variable_analysis_keeps_referenced(&project, &after)
        .expect("referenced variables must survive analysis");
    assert_eq!(after.stage.variables.len(), 1, "`g` stays");
    assert_eq!(after.sprites[0].variables.len(), 1, "`local` stays");
    assert_semantically_equal(&project, &after, "variable analysis on a fully-referenced project");
}

#[test]
fn variable_analysis_keeps_a_write_only_variable() {
    // A write is a use: a variable that is only ever written must not be
    // dropped, because deleting its declaration would break the reference.
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("counter"));
    stage.add_script(green_flag(vec![Stmt::ChangeVariable { var: "counter".into(), delta: num(1.0) }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut VariableAnalysis::new(), &project);
    assert_eq!(report.saved_variables, 0, "write-only variables are still referenced");
    variable_analysis_keeps_referenced(&project, &after).expect("analysis kept the reference intact");
    assert_eq!(after.stage.variables.len(), 1, "write-only variable survives");
    assert_semantically_equal(&project, &after, "variable analysis on a write-only variable");
}

#[test]
fn variable_analysis_may_drop_an_unreferenced_declaration_but_must_report_it() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("used"));
    stage.add_variable(var("junk"));
    stage.add_script(green_flag(vec![Stmt::SetVariable { var: "used".into(), value: num(5.0) }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut VariableAnalysis::new(), &project);
    assert_eq!(report.saved_variables, 1, "`junk` is referenced nowhere");
    variable_analysis_keeps_referenced(&project, &after)
        .expect("dropping only an unreferenced declaration is allowed");
    assert_eq!(after.stage.variables.len(), 1, "only `used` remains declared");
    assert!(
        !after.stage.variables.iter().any(|v| v.name == "junk"),
        "unreferenced `junk` removed"
    );
    assert_semantically_different(
        &project,
        &after,
        "variable analysis removing an unreferenced declaration",
    );
}

// ========================================================= empty blocks

#[test]
fn empty_block_removal_leaves_nonempty_blocks_alone() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("x"));
    stage.add_script(green_flag(vec![
        Stmt::SetVariable { var: "x".into(), value: num(1.0) },
        Stmt::If {
            condition: Expr::operator("operator_gt", vec![Expr::variable("x"), num(0.0)]),
            then_body: vec![Stmt::SetVariable { var: "y".into(), value: num(2.0) }],
            else_body: vec![],
        },
        Stmt::Repeat { times: num(3.0), body: vec![Stmt::SetVariable { var: "z".into(), value: num(3.0) }] },
    ]));
    let project = project_with(stage);

    let (after, report) = apply(&mut EmptyBlockRemoval::new(), &project);
    assert_eq!(report.deleted_nodes, 0, "no empty wrapper here");
    empty_block_removal_matches(&project, &after)
        .expect("non-empty blocks must be left alone");
    assert_semantically_equal(&project, &after, "empty-block removal on non-empty blocks");
}

#[test]
fn empty_block_removal_strips_empty_wrappers_and_reports_the_change() {
    let mut stage = Stage::new("Stage");
    stage.add_script(green_flag(vec![
        Stmt::SetVariable { var: "a".into(), value: num(1.0) },
        Stmt::If {
            condition: Expr::operator("operator_equals", vec![num(1.0), num(1.0)]),
            then_body: vec![],
            else_body: vec![],
        },
        Stmt::Repeat { times: num(3.0), body: vec![] },
        Stmt::SetVariable { var: "b".into(), value: num(2.0) },
    ]));
    let project = project_with(stage);

    let (after, report) = apply(&mut EmptyBlockRemoval::new(), &project);
    assert_eq!(report.deleted_nodes, 2, "empty If and empty Repeat removed");
    empty_block_removal_matches(&project, &after)
        .expect("only empty wrappers were removed");
    assert_eq!(
        after.stage.scripts[0].entry.body.len(),
        2,
        "the surrounding SetVariable statements survive"
    );
    assert_semantically_different(
        &project,
        &after,
        "empty-block removal deleting empty wrappers",
    );
}

#[test]
fn empty_block_removal_strips_a_nested_empty_wrapper_but_keeps_its_parent() {
    let mut stage = Stage::new("Stage");
    stage.add_variable(var("x"));
    stage.add_script(green_flag(vec![Stmt::If {
        condition: Expr::operator("operator_gt", vec![Expr::variable("x"), num(0.0)]),
        then_body: vec![
            Stmt::SetVariable { var: "a".into(), value: num(1.0) },
            Stmt::If {
                condition: Expr::operator("operator_equals", vec![num(2.0), num(2.0)]),
                then_body: vec![],
                else_body: vec![],
            },
            Stmt::SetVariable { var: "c".into(), value: num(3.0) },
        ],
        else_body: vec![],
    }]));
    let project = project_with(stage);

    let (after, report) = apply(&mut EmptyBlockRemoval::new(), &project);
    assert_eq!(report.deleted_nodes, 1, "only the nested empty If is removed");
    empty_block_removal_matches(&project, &after)
        .expect("parent If has a non-empty body and must survive");
    match &after.stage.scripts[0].entry.body[0] {
        Stmt::If { then_body, .. } => assert_eq!(then_body.len(), 2, "both neighbors survive"),
        other => panic!("expected the outer If to remain, got {other:?}"),
    }
    assert_semantically_different(
        &project,
        &after,
        "empty-block removal of a nested empty wrapper",
    );
}
