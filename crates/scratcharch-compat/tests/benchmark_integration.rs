//! Benchmark integration tests (Part 15): the committed corpus, the staged
//! runner, classification, metrics, filters, the native differential, and the
//! regression oracle — exercised end to end through `scratcharch-compat`.
//!
//! These load the committed manifest from the repository root (the workspace
//! root is two levels above this crate). No C compiler is required: the full
//! corpus run disables the native differential; the one native test skips when
//! no compiler is on PATH.

use std::path::PathBuf;

use scratcharch_compat::manifest::Manifest;
use scratcharch_compat::progress::format_progress_bar;
use scratcharch_compat::report::{build_report, render_json};
use scratcharch_compat::runner::{run_all, Corpus, RunConfig};
use scratcharch_compat::status::{Outcome, ResultClass, Stage};

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/
    p.pop(); // <workspace root>
    p
}

fn corpus() -> Corpus {
    Corpus::load(repo_root()).expect("load committed corpus manifest")
}

/// A config that never touches a C compiler (deterministic everywhere).
fn no_native() -> RunConfig {
    RunConfig {
        native: false,
        ..Default::default()
    }
}

/// v0.3 full-corpus snapshot values (must match LLVM_COMPATIBILITY_BASELINE.md).
fn assert_v03_stage_snapshot(report: &scratcharch_compat::report::Report) {
    let percent_of = |s: Stage| {
        report
            .stage_metrics
            .iter()
            .find(|m| m.stage == s)
            .expect("stage row present")
            .percent
    };
    assert_eq!(report.total, 33);
    assert_eq!(report.overall_percent, 85);
    assert_eq!(percent_of(Stage::Parser), 85);
    assert_eq!(percent_of(Stage::Sair), 85);
    assert_eq!(percent_of(Stage::Interpreter), 85);
    assert_eq!(percent_of(Stage::Vm), 79);
    assert_eq!(percent_of(Stage::Scratch), 76);
}

#[test]
fn v03_full_corpus_gate_is_green_and_snapshot_holds() {
    let outcomes = run_all(&corpus(), &no_native());
    assert_eq!(outcomes.len(), 33, "v0.3 corpus has 33 fixtures");

    // Regression oracle: no fixture drifts from its recorded expectation.
    for o in &outcomes {
        assert!(
            o.expectations_met(),
            "{} drifted from its manifest expectation: {:?}",
            o.name,
            o.violations
        );
    }

    let report = build_report(
        &outcomes,
        "0.3",
        "2026-09-09",
        None,
        &scratcharch_compat::status::REPORT_STAGES,
    );
    assert!(report.gate_green);
    assert!(report.failures.is_empty());
    assert!(report.semantic_mismatches.is_empty());
    assert_v03_stage_snapshot(&report);

    // Classification matches the v0.3 shape: every non-success fixture is a
    // *known* capability gap (frontend, runtime intrinsic, or Scratch model),
    // never a hidden correctness failure.
    assert_eq!(report.class_counts.get("success"), Some(&23));
    assert_eq!(report.class_counts.get("parse-failure"), Some(&5));
    assert_eq!(report.class_counts.get("vm-failure"), Some(&2));
    assert_eq!(report.class_counts.get("scratch-backend-failure"), Some(&3));
    assert!(!report.class_counts.contains_key("semantic-mismatch"));

    // Internal consistency: Overall == the semantic core recomputed from the
    // outcomes (interpreter passed and no value mismatch), via the shared
    // nearest-percent rounding the report uses.
    let semantic_core = outcomes
        .iter()
        .filter(|o| {
            o.mismatches.is_empty()
                && o
                    .stage_outcome(Stage::Interpreter)
                    .is_some_and(|s| s.outcome == Outcome::Pass)
        })
        .count();
    assert_eq!(
        report.overall_percent,
        scratcharch_compat::progress::percent(semantic_core, outcomes.len())
    );

    // JSON is machine-readable, has the totals, and carries no progress bars.
    let json = render_json(&report);
    assert!(!json.contains('█') && !json.contains('#'), "JSON must not carry bars");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON report");
    assert_eq!(value["total"].as_u64(), Some(33));
    assert_eq!(value["overall"].as_u64(), Some(85));
    assert_eq!(value["gate_green"].as_bool(), Some(true));
    assert_eq!(value["stages"]["scratch"].as_u64(), Some(76));
    assert_eq!(value["stages"]["vm"].as_u64(), Some(79));
}

#[test]
fn feature_filter_selects_only_tagged_fixtures() {
    let cfg = RunConfig {
        features: vec!["bitwise".to_string()],
        native: false,
        ..Default::default()
    };
    let outcomes = run_all(&corpus(), &cfg);
    assert_eq!(outcomes.len(), 1);
    let fx = &outcomes[0];
    assert_eq!(fx.name, "bitwise");
    // Correct on the reference engine, blocked at the Scratch backend only.
    assert_eq!(fx.class, ResultClass::ScratchBackendFailure);
    assert!(fx.expected_unsupported);
    assert_eq!(fx.interpreter.as_ref().and_then(|v| v.to_u64()), Some(293345));

    let float_cfg = RunConfig {
        features: vec!["float".to_string()],
        native: false,
        ..Default::default()
    };
    let floats = run_all(&corpus(), &float_cfg);
    assert_eq!(floats.len(), 1);
    assert_eq!(floats[0].name, "float");
    assert_eq!(floats[0].class, ResultClass::ParseFailure);
}

#[test]
fn boundary_filter_selects_parser_gap_fixtures() {
    let cfg = RunConfig {
        boundary: vec![Stage::Parser],
        native: false,
        ..Default::default()
    };
    let outcomes = run_all(&corpus(), &cfg);
    let names: Vec<&str> = outcomes.iter().map(|o| o.name.as_str()).collect();
    // v0.2 adds `global-agg`, the aggregate-constant global initializer gap.
    assert_eq!(names, vec!["float", "vector", "indirect-call", "atomic", "global-agg"]);
    for o in &outcomes {
        assert_eq!(o.class, ResultClass::ParseFailure);
    }
}

#[test]
fn frontier_truncation_reports_success_through_measured_stage() {
    // A stage-bound run that stops before a fixture's recorded boundary is a
    // *frontier* measurement, not a correctness failure: everything measured
    // passed, so the fixture reads as Success and records no violations.
    let cfg = RunConfig {
        stages: vec![Stage::Interpreter],
        features: vec!["phi".to_string()],
        native: false,
        ..Default::default()
    };
    let outcomes = run_all(&corpus(), &cfg);
    assert!(!outcomes.is_empty());
    for o in &outcomes {
        assert_eq!(o.class, ResultClass::Success, "{} truncated at interpreter", o.name);
        assert!(o.violations.is_empty(), "no oracle under truncation");
        assert!(o.stage_outcome(Stage::Scratch).is_none());
    }
}

#[test]
fn regression_oracle_flags_drift_from_the_manifest() {
    // A synthetic manifest that mis-records `add` as a parser capability gap.
    // The runner must *not* stay silently green: actual outcome (passes
    // everywhere, returns 42) drifts from the recorded expectation (parser
    // unsupported), surfacing a missing-unsupported violation.
    let manifest = r#"{
        "version": "0.1",
        "fixtures": [{
            "name": "add",
            "llvm_ir": "tests/c_programs/add.ll",
            "feature_tags": ["integer"],
            "expected_result": 42,
            "expected_stage": "parser",
            "expected_status": "unsupported"
        }]
    }"#;
    let parsed = Manifest::parse(manifest, repo_root()).expect("synthetic manifest valid");
    let workdir = std::env::temp_dir().join("scratcharch_compat_regression");
    let corpus = Corpus {
        manifest: parsed,
        workdir,
    };
    let outcomes = run_all(&corpus, &no_native());
    assert_eq!(outcomes.len(), 1);
    let fx = &outcomes[0];
    assert_eq!(fx.name, "add");
    assert!(!fx.expectations_met(), "must not stay green against a wrong expectation");
    assert!(
        fx.violations.iter().any(|v| v.kind == "missing-unsupported"),
        "expected a missing-unsupported violation, got {:?}",
        fx.violations
    );

    let report = build_report(
        &outcomes,
        "0.1",
        "2026-09-09",
        None,
        &scratcharch_compat::status::REPORT_STAGES,
    );
    assert!(!report.gate_green);
    assert!(!report.failures.is_empty());
}

#[test]
fn native_differential_agrees_when_a_compiler_is_present() {
    // The `switch` fixture is a single, fully-successful program returning 30
    // (it and `struct` share that result); its tag is unique to one fixture, so
    // the differential stays one-on-one after the v0.2 corpus expansion.
    let outcomes = run_all(
        &corpus(),
        &RunConfig {
            features: vec!["switch".to_string()],
            native: true,
            ..Default::default()
        },
    );
    assert_eq!(outcomes.len(), 1);
    let fx = &outcomes[0];
    assert_eq!(fx.name, "switch");
    match scratcharch_compat::native::cc_available() {
        Some(_) => {
            let native = fx.native.as_ref().expect("native ran");
            assert_eq!(native.exit_masked, 30);
            assert_eq!(fx.class, ResultClass::Success);
            assert!(fx.violations.is_empty());
        }
        None => {
            assert!(fx.native.is_none(), "no compiler, no native result");
        }
    }
}

#[test]
fn progress_bar_bounds_are_respected_by_report_bars() {
    // The one shared bar helper drives the dashboard: 0% is empty, 100% is full,
    // and any intermediate percent renders as a fixed width-20 bar (plus the
    // enclosing brackets).
    let empty = format_progress_bar(0, 20);
    let full = format_progress_bar(100, 20);
    let mid = format_progress_bar(84, 20);
    // Brackets are not block glyphs, so counting filled/empty cells over the
    // whole string is exact.
    assert_eq!(empty.chars().filter(|&c| c == '░').count(), 20);
    assert_eq!(full.chars().filter(|&c| c == '█').count(), 20);
    assert_eq!(mid.chars().filter(|&c| c == '█').count(), 17);
    assert_eq!(mid.chars().filter(|&c| c == '░').count(), 3);
    assert_eq!(mid.chars().count(), 22, "width-20 bar + brackets");
    assert_eq!(mid.chars().next(), Some('['));
    assert_eq!(mid.chars().last(), Some(']'));
}
