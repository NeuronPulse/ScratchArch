use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use scratcharch_compat::native::{cc_available, clang_version, is_clang};
use scratcharch_compat::report::{
    build_report, render_json, render_text, Report,
};
use scratcharch_compat::runner::{run_all, Corpus, RunConfig};
use scratcharch_compat::status::{Stage, FEATURES, REPORT_STAGES};

/// Options for `scratcharch test-compat`.
pub struct TestCompatArgs {
    /// Restrict to fixtures carrying one of these canonical feature tags.
    pub features: Vec<String>,
    /// Restrict to fixtures whose recorded first non-fully-expected stage
    /// (`expected_stage`) is one of these.
    pub stages: Vec<String>,
    /// Recompile every committed `.c` with clang before running.
    pub fresh_clang: bool,
    /// Skip the native differential even when a C compiler is present.
    pub no_native: bool,
    /// Report format: text | json.
    pub format: String,
    /// Compact text (headline + regressions only).
    pub quiet: bool,
    /// Verbose text (per-fixture detail).
    pub verbose: bool,
    /// Force ASCII progress bars (text only). Default: unicode on a terminal,
    /// ASCII otherwise (so logs/pipes stay portable).
    pub ascii: bool,
    /// Repo root containing `tests/corpus/llvm` (default: auto-discovered).
    pub corpus_root: Option<String>,
}

/// `scratcharch test-compat`.
///
/// Runs the LLVM compatibility benchmark over the committed corpus and prints
/// the dashboard (text) or machine report (JSON). Exits non-zero when the
/// corpus gate is red (a fixture's actual outcome drifted from its recorded
/// manifest expectation).
pub fn run(args: &TestCompatArgs) -> Result<(), String> {
    let format = args.format.to_ascii_lowercase();
    if format != "text" && format != "json" {
        return Err(format!("unknown --format {format:?} (expected text or json)"));
    }
    if args.quiet && args.verbose {
        return Err("--quiet and --verbose are mutually exclusive".to_string());
    }

    // Feature filter: tags must be part of the canonical taxonomy.
    for tag in &args.features {
        if !FEATURES.contains(&tag.as_str()) {
            return Err(format!(
                "unknown feature {tag:?} (canonical: {})",
                FEATURES.join(", ")
            ));
        }
    }

    // Boundary (`--stage`) filter: parse report-stage names.
    let boundary: Vec<Stage> = args
        .stages
        .iter()
        .map(|s| {
            let s = s.to_ascii_lowercase();
            let stage = Stage::from_str(&s);
            match stage {
                Some(stage) if REPORT_STAGES.contains(&stage) => Ok(stage),
                _ => Err(format!(
                    "unknown --stage {s:?} (expected one of: {})",
                    REPORT_STAGES
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        })
        .collect::<Result<_, _>>()?;

    let cfg = RunConfig {
        features: args.features.clone(),
        stages: Vec::new(),
        boundary,
        fresh_clang: args.fresh_clang,
        native: !args.no_native,
        ..Default::default()
    };

    let root = locate_repo_root(args.corpus_root.as_deref())?;
    let corpus = Corpus::load(&root)?;
    let outcomes = run_all(&corpus, &cfg);

    // The compiler channel shown in the header. Clang is named by version; a
    // non-clang C compiler (gcc) can drive the native differential but not
    // `--fresh-clang`.
    let clang = clang_version().or_else(|| {
        cc_available().map(|cc| format!("{cc} (native differential only)"))
    });

    let date = today();
    let mut report = build_report(
        &outcomes,
        &corpus.manifest.version,
        &date,
        clang,
        &REPORT_STAGES,
    );
    if args.fresh_clang && !is_clang() {
        report.notes.push(
            "--fresh-clang requested but clang is unavailable; used committed .ll".to_string(),
        );
    }
    if args.no_native {
        report.notes.push("native differential disabled".to_string());
    }
    if !cfg.boundary.is_empty() {
        report.notes.push(format!(
            "restricted to fixtures whose boundary stage is {}",
            cfg.boundary
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    match format.as_str() {
        "json" => println!("{}", render_json(&report)),
        _ => print!("{}", render_text_opt(&report, args)),
    }

    if report.gate_green {
        Ok(())
    } else {
        Err(format!(
            "compatibility gate FAILED: {} regression(s) against the manifest",
            report.failures.len()
        ))
    }
}

/// Text rendering honouring --quiet / --verbose / --ascii.
fn render_text_opt(report: &Report, args: &TestCompatArgs) -> String {
    if args.quiet {
        return render_quiet(report);
    }
    let unicode = !args.ascii && std::io::stdout().is_terminal();
    render_text(report, args.verbose, unicode)
}

/// The compact text report: headline, class counts, and any regressions.
fn render_quiet(report: &Report) -> String {
    use scratcharch_compat::progress::format_progress_bar_ascii;
    let mut out = String::new();
    out.push_str(&format!(
        "LLVM Compatibility v{} — Overall {:>3}%  {}\n",
        report.corpus_version,
        report.overall_percent,
        format_progress_bar_ascii(report.overall_percent, 20)
    ));
    for (class, count) in &report.class_counts {
        out.push_str(&format!("  {class}: {count}\n"));
    }
    for note in &report.notes {
        out.push_str(&format!("  note: {note}\n"));
    }
    if report.gate_green {
        out.push_str("Gate: green\n");
    } else {
        out.push_str("Regression failures:\n");
        for f in &report.failures {
            let stage = f.stage.as_deref().unwrap_or("-");
            out.push_str(&format!("  {} [{}] {}: {}\n", f.fixture, stage, f.kind, f.message));
        }
        out.push_str("Gate: FAILED\n");
    }
    out
}

/// ISO-8601 date for the report record.
fn today() -> String {
    // No chrono dependency in the CLI; format from the system date.
    let out = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok();
    match out {
        Some(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

/// Locate the repo root that contains `tests/corpus/llvm/manifest.json`.
///
/// Resolution order: an explicit `--corpus-root`, the
/// `SCRATCHARCH_COMPAT_ROOT` environment variable, an upward search from the
/// current directory, then the build-time manifest dir (`crates/scratcharch-cli`
/// → repo root).
fn locate_repo_root(explicit: Option<&str>) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(PathBuf::from(p));
    } else {
        if let Ok(p) = std::env::var("SCRATCHARCH_COMPAT_ROOT") {
            candidates.push(PathBuf::from(p));
        }
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(found) = search_up(&cwd) {
                candidates.push(found);
            }
        }
        // Build-time fallback: <workspace>/crates/scratcharch-cli → ../..
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(".."),
        );
    }
    for root in candidates {
        if let Ok(canon) = std::fs::canonicalize(&root) {
            if canon.join("tests").join("corpus").join("llvm").join("manifest.json").is_file() {
                return Ok(canon);
            }
        }
    }
    Err(
        "cannot locate the corpus manifest (tests/corpus/llvm/manifest.json); \
         pass --corpus-root <repo-root> or set SCRATCHARCH_COMPAT_ROOT"
            .to_string(),
    )
}

/// Walk upward from `start` looking for a directory containing the corpus.
fn search_up(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join("tests").join("corpus").join("llvm").join("manifest.json").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}
