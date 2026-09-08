//! The `scratcharch verify` engine.
//!
//! Given a parsed [`Project`] this composes the framework into a single
//! verdict, mirroring the pipeline ScratchArch actually runs a project
//! through. The reported checks are:
//!
//! 1. **Graph validation** — reference integrity (see [`crate::graph`]):
//!    the project is well-formed ScratchGraph.
//! 2. **Roundtrip** — writing the project to `.sb3` bytes and reading it back
//!    succeeds (no format-level failure).
//! 3. **Semantic preservation** — the roundtrip is lossless: the semantic
//!    diff between the original and the round-tripped project is empty.
//! 4. **Transform preservation** — every pass in the default pipeline, run on
//!    a copy, stays within its sound contract (the verdicts in
//!    [`crate::preservation`]). A pass that removes provably-dead code or
//!    folds a constant is *allowed*; a pass that deletes a reachable receiver,
//!    a live procedure, or changes runtime behavior is not.
//!
//! All four checks must pass for `verify` to succeed. Because the roundtrip
//! leg is SB3 write→read, a project is verified against the *native*
//! roundtrippable subset documented in SCRATCH_SEMANTICS.md; anything outside
//! it is reported, never silently blessed.

use serde::Serialize;

use scratcharch_analyzer::{semantic_diff, DiffFormat};
use scratcharch_scratchgraph::Project;
use scratcharch_transform::{
    ConstantFolding, DeadScriptElimination, EmptyBlockRemoval, TransformPass, VariableAnalysis,
};

use crate::graph::validate as validate_graph;
use crate::preservation::{
    constant_folding_reaches_canonical_fold, dce_preserves_liveness,
    empty_block_removal_matches, variable_analysis_keeps_referenced,
};
use crate::roundtrip::roundtrip_sb3;

/// Outcome of running one transform pass against its soundness contract.
#[derive(Debug, Clone, Serialize)]
pub struct PassCheck {
    pub name: String,
    /// Number of IR nodes the pass actually changed (deletions, folds, saved
    /// declarations).
    pub changes: u32,
    /// Whether the pass stayed within its sound contract (true) or committed
    /// a disallowed change (false).
    pub sound: bool,
    /// Human-readable detail when `sound` is false.
    pub note: Option<String>,
}

/// Full verification result for one project.
#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub graph_valid: bool,
    pub graph_issues: Vec<String>,
    pub roundtrip_ok: bool,
    pub roundtrip_error: Option<String>,
    pub semantic_preserved: bool,
    pub semantic_diff_text: String,
    pub passes: Vec<PassCheck>,
    pub transform_preserved: bool,
}

impl VerifyReport {
    /// All checks green.
    pub fn passed(&self) -> bool {
        self.graph_valid && self.roundtrip_ok && self.semantic_preserved && self.transform_preserved
    }

    /// The `PASS`/`FAIL` lines shown by the CLI.
    pub fn to_text(&self) -> String {
        fn status(ok: bool) -> &'static str {
            if ok { "PASS" } else { "FAIL" }
        }
        let mut out = String::new();
        out.push_str("Parse: PASS\n");
        out.push_str(&format!("Graph validation: {}\n", status(self.graph_valid)));
        out.push_str(&format!("Roundtrip: {}\n", status(self.roundtrip_ok)));
        out.push_str(&format!("Semantic preservation: {}\n", status(self.semantic_preserved)));
        for issue in &self.graph_issues {
            out.push_str(&format!("  graph issue: {issue}\n"));
        }
        if let Some(err) = &self.roundtrip_error {
            out.push_str(&format!("  roundtrip error: {err}\n"));
        }
        if !self.semantic_preserved {
            for line in self.semantic_diff_text.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out.push_str(&format!("Transform preservation: {}\n", status(self.transform_preserved)));
        for p in &self.passes {
            if !p.sound {
                if let Some(note) = &p.note {
                    out.push_str(&format!("  {}: {note}\n", p.name));
                }
            } else if p.changes > 0 {
                out.push_str(&format!("  {}: {} change(s), sound\n", p.name, p.changes));
            }
        }
        out
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn check_pass(name: &str, changes: u32, sound: Result<(), String>) -> PassCheck {
    let (ok, note) = match sound {
        Ok(()) => (true, None),
        Err(note) => (false, Some(note)),
    };
    PassCheck {
        name: name.to_string(),
        changes,
        sound: ok,
        note,
    }
}

/// Run every check against `project`. Does not mutate `project`.
pub fn verify_project(project: &Project) -> VerifyReport {
    // Graph validation.
    let (graph_valid, graph_issues) = match validate_graph(project) {
        Ok(()) => (true, Vec::new()),
        Err(issues) => (false, issues),
    };

    // Roundtrip: write .sb3 bytes, read back.
    let (roundtrip_ok, roundtrip_error, roundtripped, semantic_preserved, semantic_diff_text) =
        match roundtrip_sb3(project) {
            Err(e) => (false, Some(e), None, false, String::new()),
            Ok(back) => {
                let diff = semantic_diff(project, &back);
                let preserved = diff.is_empty();
                let text = diff.format(DiffFormat::Text);
                (true, None, Some(back), preserved, text)
            }
        };
    let _ = roundtripped;

    // Transform preservation: each default-pipeline pass against the original,
    // judged by its own soundness verdict.
    let passes = run_pass_checks(project);

    let transform_preserved = passes.iter().all(|p| p.sound);

    VerifyReport {
        graph_valid,
        graph_issues,
        roundtrip_ok,
        roundtrip_error,
        semantic_preserved,
        semantic_diff_text,
        passes,
        transform_preserved,
    }
}

fn run_pass_checks(project: &Project) -> Vec<PassCheck> {
    let mut out = Vec::new();

    let mut dce = DeadScriptElimination::new();
    let mut p1 = project.clone();
    let rep1 = dce.run(&mut p1);
    out.push(check_pass(
        "Dead Script Elimination",
        rep1.deleted_nodes,
        dce_preserves_liveness(project, &p1),
    ));

    let mut fold = ConstantFolding::new();
    let mut p2 = project.clone();
    let rep2 = fold.run(&mut p2);
    out.push(check_pass(
        "Constant Folding",
        rep2.modifications,
        constant_folding_reaches_canonical_fold(project, &p2),
    ));

    let mut vars = VariableAnalysis::new();
    let mut p3 = project.clone();
    let rep3 = vars.run(&mut p3);
    out.push(check_pass(
        "Variable Analysis",
        rep3.saved_variables,
        variable_analysis_keeps_referenced(project, &p3),
    ));

    let mut empty = EmptyBlockRemoval::new();
    let mut p4 = project.clone();
    let rep4 = empty.run(&mut p4);
    out.push(check_pass(
        "Empty Block Removal",
        rep4.deleted_nodes,
        empty_block_removal_matches(project, &p4),
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use scratcharch_scratchgraph::ir::{EventHat, Script, Stage, Stmt, Variable};

    #[test]
    fn empty_project_passes_everything() {
        let project = Project::new();
        let report = verify_project(&project);
        assert!(report.graph_valid, "{:?}", report.graph_issues);
        assert!(report.roundtrip_ok, "{:?}", report.roundtrip_error);
        assert!(report.semantic_preserved, "{}", report.semantic_diff_text);
        assert!(report.transform_preserved);
        assert!(report.passed());
    }

    #[test]
    fn live_script_project_passes_everything() {
        let mut stage = Stage::new("Stage");
        stage.add_variable(Variable::new("x_id", "x"));
        stage.add_script(Script::new(
            EventHat::GreenFlag,
            vec![Stmt::SetVariable { var: "x".into(), value: scratcharch_scratchgraph::ir::Expr::number(1.0) }],
        ));
        let project = Project::new().with_stage(stage);
        let report = verify_project(&project);
        assert!(report.graph_valid, "{:?}", report.graph_issues);
        assert!(report.semantic_preserved, "{}", report.semantic_diff_text);
        assert!(report.transform_preserved);
        assert!(report.passed());
    }

    #[test]
    fn undeclared_variable_fails_graph_validation() {
        // Referencing a variable that nothing declares is a graph violation,
        // even though it parses fine.
        let mut stage = Stage::new("Stage");
        stage.add_script(Script::new(
            EventHat::GreenFlag,
            vec![Stmt::SetVariable { var: "ghost".into(), value: scratcharch_scratchgraph::ir::Expr::number(1.0) }],
        ));
        let project = Project::new().with_stage(stage);
        let report = verify_project(&project);
        assert!(!report.graph_valid);
        assert!(
            report.graph_issues.iter().any(|i| i.contains("ghost")),
            "{:?}",
            report.graph_issues
        );
        assert!(!report.passed());
    }

    #[test]
    fn abi_lowered_code_fails_semantic_preservation() {
        // ABI frame operations are expanded into list idioms on export and not
        // reconstructed on read-back, so the *roundtrip succeeds* but semantic
        // preservation must be reported FAIL (never silently blessed). This is
        // reachable in memory; an .sb3 file can never carry ABI IR.
        let mut stage = Stage::new("Stage");
        stage.add_script(Script::new(
            EventHat::GreenFlag,
            vec![
                Stmt::EnterFrame { slots: 1 },
                Stmt::FrameSet { offset: 0, value: scratcharch_scratchgraph::ir::Expr::number(7.0) },
                Stmt::PopFrame { slots: 1 },
            ],
        ));
        let project = Project::new().with_stage(stage);
        let report = verify_project(&project);
        assert!(report.roundtrip_ok, "roundtrip itself must not fail: {:?}", report.roundtrip_error);
        assert!(!report.semantic_preserved, "{}", report.semantic_diff_text);
        assert!(!report.passed());
    }
}
