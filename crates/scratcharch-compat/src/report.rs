//! Compatibility metrics and reporting (Parts 5, 7, 8).
//!
//! A [`Report`] is computed from the per-fixture [`FixtureOutcome`]s and renders
//! two ways:
//!
//! * **text** — the human dashboard: fixed-width character bars per stage axis
//!   and feature, plus the unsupported-feature summary and any regressions;
//! * **JSON** — a machine-readable record of the same numbers (no progress bars
//!   in JSON).
//!
//! Percentages are per-stage **pass / total-corpus**, so a fixture blocked at an
//! early layer also reads as not-passed on every later layer — the honest "how
//! much of the whole corpus succeeds *through* each layer" the benchmark asks
//! for. The optimizer is an internal stage (no dashboard row). The **Overall**
//! headline is not the deepest pass-through row but the semantic core (see
//! [`build_report`]): fixtures that computed a correct result on the reference
//! interpreter, whether or not a target backend could build them.

use std::collections::BTreeMap;

use crate::progress::{format_progress_bar, format_progress_bar_ascii, percent};
use crate::runner::FixtureOutcome;
use crate::status::{FEATURES, ResultClass, Stage};

/// The fixed bar width used across the dashboard.
pub const BAR_WIDTH: usize = 20;

#[derive(Debug, Clone)]
pub struct StageMetrics {
    pub stage: Stage,
    pub pass: usize,
    pub unsupported: usize,
    pub fail: usize,
    pub not_measured: usize,
    pub percent: u32,
}

#[derive(Debug, Clone)]
pub struct FeatureMetrics {
    pub feature: String,
    pub total: usize,
    pub success: usize,
    pub unsupported: usize,
    pub mismatches: usize,
    pub percent: u32,
}

#[derive(Debug, Clone)]
pub struct FailureRecord {
    pub fixture: String,
    pub stage: Option<String>,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct MismatchRecord {
    pub fixture: String,
    pub channel: String,
    pub expected: String,
    pub actual: String,
}

/// Per-fixture data kept in the report for verbose text and JSON.
#[derive(Debug, Clone)]
pub struct FixtureSummary {
    pub name: String,
    pub class: &'static str,
    pub fresh_ir: bool,
    pub interpreter: Option<String>,
    pub vm: Option<String>,
    pub native_exit: Option<u32>,
    pub stages: Vec<(String, &'static str, Option<String>)>,
    pub mismatches: Vec<(String, String, String)>,
    pub violations: Vec<(Option<String>, String, String)>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub corpus_version: String,
    pub date: String,
    pub clang: Option<String>,
    pub total: usize,
    pub overall_percent: u32,
    pub success_count: usize,
    pub class_counts: BTreeMap<&'static str, usize>,
    pub stage_metrics: Vec<StageMetrics>,
    pub features: Vec<FeatureMetrics>,
    /// Known capability gaps by feature (`feature`, fixture count), desc.
    pub unsupported_features: Vec<(String, usize)>,
    pub failures: Vec<FailureRecord>,
    pub semantic_mismatches: Vec<MismatchRecord>,
    pub gate_green: bool,
    pub fixtures: Vec<FixtureSummary>,
    /// Measurement notes (e.g. clang availability).
    pub notes: Vec<String>,
}

/// Compute a [`Report`] from per-fixture outcomes.
pub fn build_report(
    outcomes: &[FixtureOutcome],
    version: &str,
    date: &str,
    clang: Option<String>,
    stages_reported: &[Stage],
) -> Report {
    let total = outcomes.len();
    let success_count = outcomes
        .iter()
        .filter(|o| o.class == ResultClass::Success)
        .count();
    // Overall is the *semantic core*: fixtures whose program computed a correct
    // result on the reference engine (the interpreter passed, with no value
    // disagreement against the expected result, native, or the VM). A fixture
    // that is correct on the interpreter but blocked at a *target* backend (the
    // ISA VM or Scratch) still counts here — it computed the right answer, it
    // just could not be built for that target. This is deliberately distinct
    // from the per-stage pass-through rows below: it is the headline that says
    // "the toolchain is semantically faithful for X% of the corpus", while the
    // backend rows quantify where the extra layers lose reach.
    let semantic_core = outcomes
        .iter()
        .filter(|o| {
            o.mismatches.is_empty()
                && o
                    .stage_outcome(crate::status::Stage::Interpreter)
                    .is_some_and(|s| s.outcome == crate::status::Outcome::Pass)
        })
        .count();
    let overall_percent = percent(semantic_core, total);

    let mut class_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for o in outcomes {
        *class_counts.entry(o.class.as_str()).or_insert(0) += 1;
    }

    let stage_metrics = stages_reported
        .iter()
        .map(|&s| {
            let mut pass = 0;
            let mut unsupported = 0;
            let mut fail = 0;
            let mut not_measured = 0;
            for o in outcomes {
                match o.stage_outcome(s).map(|x| x.outcome) {
                    Some(crate::status::Outcome::Pass) => pass += 1,
                    Some(crate::status::Outcome::Unsupported) => unsupported += 1,
                    Some(crate::status::Outcome::Fail) => fail += 1,
                    None => not_measured += 1,
                }
            }
            StageMetrics {
                stage: s,
                pass,
                unsupported,
                fail,
                not_measured,
                percent: percent(pass, total),
            }
        })
        .collect();

    let mut features: Vec<FeatureMetrics> = Vec::new();
    for &feat in FEATURES {
        let tagged: Vec<&FixtureOutcome> =
            outcomes.iter().filter(|o| o.feature_tags.iter().any(|t| t == feat)).collect();
        if tagged.is_empty() {
            continue;
        }
        let total_f = tagged.len();
        let success = tagged.iter().filter(|o| o.class == ResultClass::Success).count();
        let unsupported = tagged
            .iter()
            .filter(|o| o.expected_unsupported)
            .count();
        let mismatches = tagged
            .iter()
            .filter(|o| o.class == ResultClass::SemanticMismatch)
            .count();
        features.push(FeatureMetrics {
            feature: feat.to_string(),
            total: total_f,
            success,
            unsupported,
            mismatches,
            percent: percent(success, total_f),
        });
    }

    // Known capability gaps by feature, descending by fixture count.
    let mut unsupported_features: Vec<(String, usize)> = Vec::new();
    for &feat in FEATURES {
        let n = outcomes
            .iter()
            .filter(|o| o.expected_unsupported && o.feature_tags.iter().any(|t| t == feat))
            .count();
        if n > 0 {
            unsupported_features.push((feat.to_string(), n));
        }
    }
    unsupported_features.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut failures: Vec<FailureRecord> = Vec::new();
    let mut semantic_mismatches: Vec<MismatchRecord> = Vec::new();
    let mut fixtures: Vec<FixtureSummary> = Vec::new();
    for o in outcomes {
        for v in &o.violations {
            failures.push(FailureRecord {
                fixture: o.name.clone(),
                stage: v.stage.map(|s| s.as_str().to_string()),
                kind: v.kind.to_string(),
                message: v.message.clone(),
            });
        }
        for m in &o.mismatches {
            semantic_mismatches.push(MismatchRecord {
                fixture: o.name.clone(),
                channel: m.channel.to_string(),
                expected: m.expected.clone(),
                actual: m.actual.clone(),
            });
        }
        fixtures.push(FixtureSummary {
            name: o.name.clone(),
            class: o.class.as_str(),
            fresh_ir: o.fresh_ir,
            interpreter: o.interpreter.as_ref().map(|v| format!("{v:?}")),
            vm: o.vm.as_ref().map(|v| format!("{v:?}")),
            native_exit: o.native.as_ref().map(|n| n.exit_masked),
            stages: o
                .stages
                .iter()
                .map(|s| (s.stage.as_str().to_string(), s.outcome.as_str(), s.message.clone()))
                .collect(),
            mismatches: o
                .mismatches
                .iter()
                .map(|m| (m.channel.to_string(), m.expected.clone(), m.actual.clone()))
                .collect(),
            violations: o
                .violations
                .iter()
                .map(|v| {
                    (
                        v.stage.map(|s| s.as_str().to_string()),
                        v.kind.to_string(),
                        v.message.clone(),
                    )
                })
                .collect(),
        });
    }

    let mut notes = Vec::new();
    if clang.is_none() {
        notes.push("no C compiler on PATH: fresh-clang and native differential skipped".to_string());
    } else if outcomes.iter().any(|o| o.fresh_ir) {
        notes.push("fixtures recompiled fresh with clang".to_string());
    }

    let gate_green = failures.is_empty();
    Report {
        corpus_version: version.to_string(),
        date: date.to_string(),
        clang,
        total,
        overall_percent,
        success_count,
        class_counts,
        stage_metrics,
        features,
        unsupported_features,
        failures,
        semantic_mismatches,
        gate_green,
        fixtures,
        notes,
    }
}

fn stage_label_row(percent: u32, unicode: bool) -> String {
    let bar = if unicode {
        format_progress_bar(percent, BAR_WIDTH)
    } else {
        format_progress_bar_ascii(percent, BAR_WIDTH)
    };
    format!("{percent:>3}%  {bar}")
}

/// Render the dashboard text report.
pub fn render_text(report: &Report, verbose: bool, unicode: bool) -> String {
    let mut out = String::new();
    out.push_str("LLVM Compatibility\n");
    if verbose {
        out.push_str(&format!("corpus v{}\n", report.corpus_version));
        out.push_str(&format!(
            "clang: {}\n",
            report.clang.as_deref().unwrap_or("(none)")
        ));
    }

    // Groups: (heading, [stages]).
    let groups: [(&str, &[Stage]); 3] = [
        ("Frontend", &[Stage::Parser, Stage::Sair]),
        ("Execution", &[Stage::Interpreter, Stage::Vm]),
        ("Targets", &[Stage::Scratch]),
    ];

    // Overall only meaningful when the full pipeline was measured.
    if stage_has(&report.stage_metrics, Stage::Scratch) {
        out.push_str("\nOverall\n");
        out.push_str(&format!(
            "{}%  {}\n",
            report.overall_percent,
            bar(report.overall_percent, unicode)
        ));
    }

    let metric_of = |s: Stage| report.stage_metrics.iter().find(|m| m.stage == s);

    for (heading, stages) in groups {
        let present: Vec<&StageMetrics> =
            stages.iter().filter_map(|s| metric_of(*s)).collect();
        if present.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(heading);
        out.push('\n');
        for m in present {
            out.push_str(&format!("{}\n", m.stage.label()));
            out.push_str(&format!(
                "{}\n",
                stage_label_row(m.percent, unicode)
            ));
        }
    }

    if !report.features.is_empty() {
        out.push_str("\nFeatures\n");
        for f in &report.features {
            let bar = if unicode {
                format_progress_bar(f.percent, BAR_WIDTH)
            } else {
                format_progress_bar_ascii(f.percent, BAR_WIDTH)
            };
            out.push_str(&format!(
                "{:<14}{:>4}%  {bar}\n",
                f.feature, f.percent
            ));
        }
    }

    if !report.unsupported_features.is_empty() {
        out.push_str("\nUnsupported features:\n");
        for (feat, count) in &report.unsupported_features {
            out.push_str(&format!("{feat:<14}{count:>6}\n"));
        }
    }

    if !report.semantic_mismatches.is_empty() {
        out.push_str("\nSemantic mismatches:\n");
        for m in &report.semantic_mismatches {
            out.push_str(&format!(
                "  {} [{}]: expected {}, got {}\n",
                m.fixture, m.channel, m.expected, m.actual
            ));
        }
    }

    if !report.failures.is_empty() {
        out.push_str("\nRegression failures:\n");
        for f in &report.failures {
            let stage = f.stage.as_deref().unwrap_or("-");
            out.push_str(&format!("  {} [{}] {}: {}\n", f.fixture, stage, f.kind, f.message));
        }
        out.push_str("\nGate: FAILED\n");
    } else if verbose {
        out.push_str("\nGate: green\n");
    }

    for note in &report.notes {
        if verbose {
            out.push_str(&format!("note: {note}\n"));
        }
    }

    if verbose {
        out.push_str("\nPer-fixture:\n");
        for fx in &report.fixtures {
            out.push_str(&format!(
                "  {:<14}{}\n",
                fx.name, fx.class
            ));
            for (stage, outcome, msg) in &fx.stages {
                if *outcome != "pass" {
                    out.push_str(&format!(
                        "    {stage}: {outcome}{}\n",
                        msg.as_deref().map(|m| format!(": {m}")).unwrap_or_default()
                    ));
                }
            }
        }
    }
    out
}

/// Render the machine-readable JSON report (no progress bars).
pub fn render_json(report: &Report) -> String {
    let mut stages = serde_json::Map::new();
    for m in &report.stage_metrics {
        stages.insert(m.stage.as_str().to_string(), serde_json::json!(m.percent));
    }
    let mut features = serde_json::Map::new();
    for f in &report.features {
        features.insert(
            f.feature.clone(),
            serde_json::json!({
                "total": f.total,
                "success": f.success,
                "unsupported": f.unsupported,
                "semantic_mismatches": f.mismatches,
                "percent": f.percent,
            }),
        );
    }
    let mut unsupported = serde_json::Map::new();
    for (feat, count) in &report.unsupported_features {
        unsupported.insert(feat.clone(), serde_json::json!(count));
    }

    let failures: Vec<serde_json::Value> = report
        .failures
        .iter()
        .map(|f| {
            serde_json::json!({
                "fixture": f.fixture,
                "stage": f.stage,
                "category": f.kind,
                "message": f.message,
            })
        })
        .collect();

    let semantic: Vec<serde_json::Value> = report
        .semantic_mismatches
        .iter()
        .map(|m| {
            serde_json::json!({
                "fixture": m.fixture,
                "channel": m.channel,
                "expected": m.expected,
                "actual": m.actual,
            })
        })
        .collect();

    let fixtures: Vec<serde_json::Value> = report
        .fixtures
        .iter()
        .map(|fx| {
            serde_json::json!({
                "name": fx.name,
                "class": fx.class,
                "interpreter": fx.interpreter,
                "vm": fx.vm,
                "native_exit": fx.native_exit,
                "fresh_ir": fx.fresh_ir,
                "stages": fx.stages.iter().map(|(s, o, msg)| serde_json::json!({
                    "stage": s, "status": o, "message": msg,
                })).collect::<Vec<_>>(),
                "violations": fx.violations,
            })
        })
        .collect();

    let value = serde_json::json!({
        "corpus_version": report.corpus_version,
        "date": report.date,
        "clang": report.clang,
        "total": report.total,
        "overall": report.overall_percent,
        "stages": stages,
        "features": features,
        "unsupported_features": unsupported,
        "classes": report.class_counts,
        "failures": failures,
        "semantic_mismatches": semantic,
        "gate_green": report.gate_green,
        "fixtures": fixtures,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn stage_has(metrics: &[StageMetrics], s: Stage) -> bool {
    metrics.iter().any(|m| m.stage == s)
}

fn bar(percent: u32, unicode: bool) -> String {
    if unicode {
        format_progress_bar(percent, BAR_WIDTH)
    } else {
        format_progress_bar_ascii(percent, BAR_WIDTH)
    }
}

/// A human line used by the baseline generator.
pub fn render_baseline_lines(report: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!("Overall\n{}%\n", report.overall_percent));
    for m in &report.stage_metrics {
        out.push_str(&format!("{}\n{}%\n", m.stage.label(), m.percent));
    }
    out.push_str("Feature breakdown\n");
    for f in &report.features {
        out.push_str(&format!("{} {}\n", f.feature, f.percent));
    }
    out
}
