//! Unified compatibility runner (Part 2).
//!
//! Walks each fixture through the layered toolchain one stage at a time and
//! records a PASS / FAIL / UNSUPPORTED outcome per stage:
//!
//! ```text
//! .ll (or fresh .c → clang) → LLVM parser → SAIR (translate + validate)
//!        → optimizer → SAIR interpreter → ISA lowering → ISA VM
//!        → ScratchGraph backend
//! ```
//!
//! Errors are never swallowed: every non-pass stage keeps its diagnostic, and
//! the value-returning engines are cross-checked full-width against each other
//! and the manifest's `expected_result` (native compares masked, since its
//! channel is the process exit status by interface). The manifest doubles as
//! the regression oracle: a fixture whose actual outcome drifts from its
//! recorded expectation is surfaced as a [`Violation`] (Part 10).

use std::fs;
use std::path::PathBuf;

use scratcharch_core::value::Value as IsaValue;
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::r#module::{IrModule, STATIC_DATA_BASE};
use scratcharch_ir::types::IrType;
use scratcharch_llvm::errors::LlvmError;
use scratcharch_llvm::{parser, translator};
use scratcharch_opt::constant_fold::ConstantFold;
use scratcharch_opt::dce::DeadCodeElimination;
use scratcharch_opt::manager::PassManager;
use scratcharch_sair_interpreter::{InterpError, Interpreter, RuntimeValue};
use scratcharch_scratchgraph::ScratchGraphLowerer;
use scratcharch_target::profile::TargetProfile;
use scratcharch_vm::vm::Vm;

use crate::manifest::{BackendExpect, FixtureEntry, Manifest};
use crate::native::{compile_fresh, run_native, cc_available, NativeResult};
use crate::status::{Outcome, ResultClass, Stage, REPORT_STAGES};

const MEMORY_SIZE: u32 = 65536;
const STACK_LIMIT: u32 = 4096;
const MAX_FRAMES: usize = 1024;

/// A value a runnable engine produced, normalized across interpreter and VM.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineValue {
    I1(bool),
    I8(u8),
    I16(u16),
    I32(u32),
    I64(u64),
    F64(f64),
    Pointer(u32),
}

impl EngineValue {
    /// Numeric view used for result-channel comparison (None for floats).
    pub fn to_u64(&self) -> Option<u64> {
        match self {
            EngineValue::I1(b) => Some(*b as u64),
            EngineValue::I8(v) => Some(*v as u64),
            EngineValue::I16(v) => Some(*v as u64),
            EngineValue::I32(v) => Some(*v as u64),
            EngineValue::I64(v) => Some(*v),
            EngineValue::F64(_) => None,
            EngineValue::Pointer(p) => Some(*p as u64),
        }
    }

    /// The masked process-exit view (native-comparable).
    pub fn to_masked(&self) -> Option<u32> {
        self.to_u64().map(|v| (v & 0xFF) as u32)
    }
}

impl From<&RuntimeValue> for EngineValue {
    fn from(v: &RuntimeValue) -> Self {
        match v {
            RuntimeValue::I1(b) => EngineValue::I1(*b),
            RuntimeValue::I8(x) => EngineValue::I8(*x),
            RuntimeValue::I16(x) => EngineValue::I16(*x),
            RuntimeValue::I32(x) => EngineValue::I32(*x),
            RuntimeValue::I64(x) => EngineValue::I64(*x),
            RuntimeValue::F64(x) => EngineValue::F64(*x),
            RuntimeValue::Pointer(p) => EngineValue::Pointer(*p),
        }
    }
}

impl From<&IsaValue> for EngineValue {
    fn from(v: &IsaValue) -> Self {
        match v {
            IsaValue::I1(b) => EngineValue::I1(*b),
            IsaValue::I8(x) => EngineValue::I8(*x),
            IsaValue::I16(x) => EngineValue::I16(*x),
            IsaValue::I32(x) => EngineValue::I32(*x),
            IsaValue::F64(x) => EngineValue::F64(*x),
            IsaValue::Pointer(p) => EngineValue::Pointer(*p),
        }
    }
}

/// One recorded stage outcome. Carries the precise [`ResultClass`] the fixture
/// would take if this stage were the first non-pass, so lowering failures and
/// VM-execution failures stay distinct.
#[derive(Debug, Clone)]
pub struct StageOutcome {
    pub stage: Stage,
    pub outcome: Outcome,
    pub class: ResultClass,
    /// Preserved diagnostic (never swallowed).
    pub message: Option<String>,
}

impl StageOutcome {
    fn pass(stage: Stage) -> Self {
        StageOutcome {
            stage,
            outcome: Outcome::Pass,
            class: class_for_stage(stage),
            message: None,
        }
    }

    fn blocked(stage: Stage, outcome: Outcome, class: ResultClass, message: impl Into<String>) -> Self {
        StageOutcome {
            stage,
            outcome,
            class,
            message: Some(message.into()),
        }
    }
}

/// A correctness disagreement between engines or against the expected result.
#[derive(Debug, Clone)]
pub struct Mismatch {
    pub channel: &'static str,
    pub expected: String,
    pub actual: String,
}

/// A regression: the fixture's actual outcome drifts from its recorded
/// manifest expectation.
#[derive(Debug, Clone)]
pub struct Violation {
    pub fixture: String,
    pub stage: Option<Stage>,
    /// Stable kind: `semantic-mismatch`, `native-mismatch`, `unexpected-block`,
    /// `unexpected-success`, `diagnostic-mismatch`, `missing-unsupported`,
    /// `not-measured`.
    pub kind: &'static str,
    pub message: String,
}

/// Full per-fixture benchmark result.
#[derive(Debug, Clone)]
pub struct FixtureOutcome {
    pub name: String,
    pub stages: Vec<StageOutcome>,
    pub class: ResultClass,
    pub interpreter: Option<EngineValue>,
    pub vm: Option<EngineValue>,
    pub native: Option<NativeResult>,
    /// Whether the committed `.ll` was replaced by a fresh clang compile.
    pub fresh_ir: bool,
    pub mismatches: Vec<Mismatch>,
    pub violations: Vec<Violation>,
    /// Canonical feature tags (copied from the manifest entry).
    pub feature_tags: Vec<String>,
    /// The fixture's recorded boundary stage.
    pub expected_stage: Stage,
    /// Whether the fixture is a known capability gap (`expected_status`).
    pub expected_unsupported: bool,
}

impl FixtureOutcome {
    pub fn stage_outcome(&self, stage: Stage) -> Option<&StageOutcome> {
        self.stages.iter().find(|s| s.stage == stage)
    }

    /// `true` when the fixture satisfies its manifest expectation (gate green).
    pub fn expectations_met(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Configuration for a benchmark run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Canonical feature tags to restrict to (empty = all fixtures).
    pub features: Vec<String>,
    /// Report stages to measure (empty = full pipeline through Scratch). The
    /// deepest selected stage bounds how far each fixture is executed.
    pub stages: Vec<Stage>,
    /// Fixture filter: only fixtures whose recorded first non-fully-expected
    /// stage (`expected_stage` in the manifest) is one of these. Empty = no
    /// boundary filter. Complementary to `features` and does not change
    /// measurement depth (a full-pipeline run still measures every stage).
    pub boundary: Vec<Stage>,
    /// Recompile every `.c` with clang before running (committed `.ll` is the
    /// fallback when clang is unavailable).
    pub fresh_clang: bool,
    /// Run the native differential for fixtures that reach the interpreter.
    pub native: bool,
    pub profile: TargetProfile,
    /// Maximum call frames for the interpreter.
    pub max_frames: usize,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig {
            features: Vec::new(),
            stages: Vec::new(),
            boundary: Vec::new(),
            fresh_clang: false,
            native: true,
            profile: TargetProfile::sa48(),
            max_frames: MAX_FRAMES,
        }
    }
}

impl RunConfig {
    /// The deepest report stage to execute.
    pub fn max_stage(&self) -> Stage {
        self.stages.iter().copied().max().unwrap_or(Stage::Scratch)
    }

    /// Whether a fixture carrying `tags` is selected by the feature filter.
    pub fn fixture_selected(&self, tags: &[String]) -> bool {
        if self.features.is_empty() {
            return true;
        }
        tags.iter().any(|t| self.features.iter().any(|f| f == t))
    }

    /// Whether a fixture's recorded boundary stage (`expected_stage`) matches
    /// the `--stage` filter. A fixture with no boundary defaults to `scratch`.
    pub fn boundary_matches(&self, expected_stage: &Option<String>) -> bool {
        if self.boundary.is_empty() {
            return true;
        }
        let stage = expected_stage
            .as_deref()
            .and_then(Stage::from_str)
            .unwrap_or(Stage::Scratch);
        self.boundary.contains(&stage)
    }
}

/// A loaded corpus ready to run.
pub struct Corpus {
    pub manifest: Manifest,
    /// Native work directory (reused across fixtures).
    pub workdir: PathBuf,
}

impl Corpus {
    /// Load `tests/corpus/llvm/manifest.json` under `repo_root`.
    pub fn load(repo_root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = repo_root.into();
        let path = root.join("tests").join("corpus").join("llvm").join("manifest.json");
        let manifest = Manifest::load(&path, &root)?;
        let workdir = std::env::temp_dir().join("scratcharch_compat_native");
        Ok(Corpus { manifest, workdir })
    }

    /// Selected fixtures (manifest order), filtered by feature tags and the
    /// optional `--stage` boundary filter.
    pub fn selected(&self, cfg: &RunConfig) -> Vec<&FixtureEntry> {
        self.manifest
            .fixtures
            .iter()
            .filter(|f| cfg.fixture_selected(&f.feature_tags))
            .filter(|f| cfg.boundary_matches(&f.expected_stage))
            .collect()
    }
}

/// Run the whole selected corpus; one outcome per fixture.
pub fn run_all(corpus: &Corpus, cfg: &RunConfig) -> Vec<FixtureOutcome> {
    let cc = if cfg.native { cc_available() } else { None };
    corpus
        .selected(cfg)
        .into_iter()
        .map(|fx| run_fixture(fx, cfg, corpus, cc))
        .collect()
}

/// Run a single fixture through the staged pipeline.
fn run_fixture(fx: &FixtureEntry, cfg: &RunConfig, corpus: &Corpus, cc: Option<&str>) -> FixtureOutcome {
    let mut stages: Vec<StageOutcome> = Vec::new();
    let mut mismatches: Vec<Mismatch> = Vec::new();
    let max_stage = cfg.max_stage();
    let expect = fx.expected_result;
    // The regression oracle (manifest expectations) applies only when the run
    // measures the whole pipeline through Scratch.
    let full_pipeline = max_stage == Stage::Scratch;

    let (ir, fresh_ir) = read_ir(fx, cfg, corpus);

    // ---- Stage 1: LLVM parser ----
    let program = match parser::parse_llvm(&ir) {
        Ok(p) => {
            stages.push(StageOutcome::pass(Stage::Parser));
            p
        }
        Err(e) => {
            let (outcome, msg) = classify_llvm_error(&e);
            stages.push(StageOutcome::blocked(Stage::Parser, outcome, ResultClass::ParseFailure, msg));
            return finalize(
                fx,
                full_pipeline,
                stages,
                mismatches,
                RunValues { interpreter: None, vm: None, native: None, fresh_ir },
            );
        }
    };

    // ---- Stage 2: SAIR (translate + validate) ----
    let mut module = match translator::translate(&program) {
        Ok(m) => m,
        Err(e) => {
            let (outcome, msg) = classify_llvm_error(&e);
            stages.push(StageOutcome::blocked(Stage::Sair, outcome, ResultClass::SairFailure, msg));
            return finalize(
                fx,
                full_pipeline,
                stages,
                mismatches,
                RunValues { interpreter: None, vm: None, native: None, fresh_ir },
            );
        }
    };
    if let Err(e) = module.validate() {
        stages.push(StageOutcome::blocked(
            Stage::Sair,
            Outcome::Fail,
            ResultClass::SairFailure,
            format!("validation: {e}"),
        ));
        return finalize(
            fx,
            full_pipeline,
            stages,
            mismatches,
            RunValues { interpreter: None, vm: None, native: None, fresh_ir },
        );
    }
    stages.push(StageOutcome::pass(Stage::Sair));

    // ---- Stage 3: optimizer (basic fold + DCE) ----
    {
        let mut pm = PassManager::new();
        pm.add(ConstantFold);
        pm.add(DeadCodeElimination);
        pm.run(&mut module);
        stages.push(StageOutcome::pass(Stage::Optimizer));
    }

    // ---- Stage 4: interpreter (the semantic reference) ----
    let mut interp_value: Option<EngineValue> = None;
    if Stage::Interpreter <= max_stage {
        let mut interp = Interpreter::new(module.clone(), MEMORY_SIZE, STACK_LIMIT);
        interp.set_max_frames(cfg.max_frames);
        match interp.run() {
            Ok(v) => {
                interp_value = v.as_ref().map(EngineValue::from);
                stages.push(StageOutcome::pass(Stage::Interpreter));
            }
            Err(e) => {
                let (outcome, msg) = classify_interp_error(&e);
                stages.push(StageOutcome::blocked(Stage::Interpreter, outcome, ResultClass::InterpreterFailure, msg));
            }
        }
    }

    // ---- Stage 5: ISA lowering + VM ----
    let mut vm_value: Option<EngineValue> = None;
    if Stage::Vm <= max_stage {
        match lower_and_run_vm(&module, cfg) {
            Ok(v) => {
                vm_value = v;
                stages.push(StageOutcome::pass(Stage::Vm));
            }
            Err((class, msg)) => {
                stages.push(StageOutcome::blocked(Stage::Vm, Outcome::Unsupported, class, msg));
            }
        }
    }

    // ---- Stage 6: ScratchGraph backend ----
    if Stage::Scratch <= max_stage {
        match ScratchGraphLowerer::new().lower(&module) {
            Ok(_project) => {
                stages.push(StageOutcome::pass(Stage::Scratch));
            }
            Err(e) => {
                stages.push(StageOutcome::blocked(
                    Stage::Scratch,
                    Outcome::Unsupported,
                    ResultClass::ScratchBackendFailure,
                    e.to_string(),
                ));
            }
        }
    }

    // ---- Native differential (only where a cross-check is possible) ----
    let mut native: Option<NativeResult> = None;
    if let Some(cc) = cc {
        if let (Some(src), true) = (&fx.source, interp_value.is_some()) {
            let src_path = corpus.manifest.resolve(src);
            if src_path.exists() {
                native = run_native(&src_path, cc, &corpus.workdir).ok();
            }
        }
    }

    // ---- Correctness mismatches (full-width wherever possible) ----
    if let (Some(exp), Some(iv)) = (expect, interp_value.as_ref()) {
        if let Some(full) = iv.to_u64() {
            if full != exp {
                mismatches.push(Mismatch {
                    channel: "interpreter",
                    expected: exp.to_string(),
                    actual: format!("{iv:?}"),
                });
            }
        }
        if let Some(n) = &native {
            if n.exit_masked != (exp & 0xFF) as u32 {
                mismatches.push(Mismatch {
                    channel: "native",
                    expected: format!("exit {}", exp & 0xFF),
                    actual: format!("exit {}", n.exit_masked),
                });
            }
        }
    }
    if let (Some(iv), Some(vv)) = (interp_value.as_ref(), vm_value.as_ref()) {
        if let (Some(a), Some(b)) = (iv.to_u64(), vv.to_u64()) {
            if a != b {
                mismatches.push(Mismatch {
                    channel: "vm",
                    expected: format!("{iv:?}"),
                    actual: format!("{vv:?}"),
                });
            }
        }
    }

    finalize(
        fx,
        full_pipeline,
        stages,
        mismatches,
        RunValues {
            interpreter: interp_value,
            vm: vm_value,
            native,
            fresh_ir,
        },
    )
}

/// Read committed `.ll` (fresh clang compile first when requested).
fn read_ir(fx: &FixtureEntry, cfg: &RunConfig, corpus: &Corpus) -> (String, bool) {
    if cfg.fresh_clang {
        if let Some(src) = &fx.source {
            let src_path = corpus.manifest.resolve(src);
            if let Some(ir) = compile_fresh(&src_path) {
                return (ir, true);
            }
        }
    }
    let rel = fx
        .llvm_ir
        .as_ref()
        .expect("manifest validation guarantees llvm_ir or source");
    let path = corpus.manifest.resolve(rel);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {}: cannot read {}: {e}", fx.name, path.display()));
    (text, false)
}

/// Lower an optimized module to ISA and run it on the VM, returning the
/// top-of-stack value (the `main` result). Fails with the VM-outcome class.
fn lower_and_run_vm(
    module: &IrModule,
    cfg: &RunConfig,
) -> Result<Option<EngineValue>, (ResultClass, String)> {
    let lowerer = IsaLowerer::with_profile(cfg.profile.clone());
    let program = lowerer
        .lower(module)
        .map_err(|e| (ResultClass::LoweringFailure, format!("{e:?}")))?;

    let mut vm = Vm::new(MEMORY_SIZE, STACK_LIMIT);
    vm.load_program(&program)
        .map_err(|e| (ResultClass::VmFailure, e.to_string()))?;
    seed_vm_static(&mut vm, module, STACK_LIMIT).map_err(|e| (ResultClass::VmFailure, e))?;
    vm.run().map_err(|e| (ResultClass::VmFailure, e.to_string()))?;
    Ok(vm_result_value(&vm, module))
}

/// Reconstruct the entry function's return value from the VM operand stack.
///
/// An `i64` result is two 32-bit limb cells with the low limb below the high
/// limb (high on top); every other scalar return occupies one cell. A `void`
/// entry leaves nothing to read.
fn vm_result_value(vm: &Vm, module: &IrModule) -> Option<EngineValue> {
    let entry = module.functions.iter().find(|f| f.name == module.entry)?;
    let cell = |idx: usize| -> Option<u64> {
        EngineValue::from(vm.stack.get(idx).ok()?).to_u64()
    };
    match entry.return_ty {
        IrType::I64 => {
            let hi = cell(0)?;
            let lo = cell(1)?;
            Some(EngineValue::I64((hi << 32) | lo))
        }
        // Every other scalar return occupies one cell; a void entry leaves
        // nothing (peek underflows -> None).
        _ => vm.stack.peek().ok().map(EngineValue::from),
    }
}

/// Seed the VM static-data segment byte-exactly (mirrors the driver).
fn seed_vm_static(vm: &mut Vm, module: &IrModule, stack_limit: u32) -> Result<(), String> {
    let image = &module.static_data.image;
    if image.is_empty() {
        return Ok(());
    }
    let end = STATIC_DATA_BASE as usize + image.len();
    if end > stack_limit as usize {
        return Err(format!(
            "global static data ({} bytes at base {}) does not fit below the stack floor ({})",
            image.len(),
            STATIC_DATA_BASE,
            stack_limit
        ));
    }
    vm.memory
        .write(STATIC_DATA_BASE, image)
        .map_err(|e| format!("seeding static data failed: {e:?}"))
}

fn class_for_stage(s: Stage) -> ResultClass {
    match s {
        Stage::Parser => ResultClass::ParseFailure,
        Stage::Sair => ResultClass::SairFailure,
        Stage::Optimizer => ResultClass::OptimizationFailure,
        Stage::Interpreter => ResultClass::InterpreterFailure,
        Stage::Vm => ResultClass::VmFailure,
        Stage::Scratch => ResultClass::ScratchBackendFailure,
    }
}

/// Derive the fixture result class from stage outcomes and value mismatches.
///
/// Stages are only recorded when they were attempted. A stage missing because
/// the run was truncated by a `--stage` filter (never attempted) is not a
/// failure: every stage *before* it measured Pass, so the fixture succeeded
/// through the measured frontier and reads as [`ResultClass::Success`]. A stage
/// missing after a real early block (parser/SAIR failure) is never reached,
/// because the blocking stage is itself recorded and returned first.
fn derive_class(stages: &[StageOutcome]) -> ResultClass {
    let outcome_of = |s: Stage| stages.iter().find(|x| x.stage == s);
    // An optimizer failure is reported before any downstream stage.
    if let Some(o) = outcome_of(Stage::Optimizer) {
        if o.outcome != Outcome::Pass {
            return o.class;
        }
    }
    for stage in REPORT_STAGES {
        match outcome_of(stage) {
            Some(o) if o.outcome == Outcome::Pass => continue,
            Some(o) => return o.class,
            // Frontier truncation: everything that was measured passed.
            None => return ResultClass::Success,
        }
    }
    ResultClass::Success
}

/// Engine and native results collected along the run, handed to [`finalize`].
struct RunValues {
    interpreter: Option<EngineValue>,
    vm: Option<EngineValue>,
    native: Option<NativeResult>,
    fresh_ir: bool,
}

/// Assemble a fixture outcome, deriving class and regression violations.
///
/// The regression oracle is only meaningful for a full-pipeline run (through
/// Scratch): `expected_stage` semantics assume every report stage is measured,
/// so a stage-filtered run records no violations and lets the class reflect the
/// measured frontier.
fn finalize(
    fx: &FixtureEntry,
    full_pipeline: bool,
    stages: Vec<StageOutcome>,
    mismatches: Vec<Mismatch>,
    values: RunValues,
) -> FixtureOutcome {
    let RunValues {
        interpreter: interp_value,
        vm: vm_value,
        native,
        fresh_ir,
    } = values;
    let class = if mismatches.is_empty() {
        derive_class(&stages)
    } else {
        ResultClass::SemanticMismatch
    };
    let expected_stage = Stage::from_str(fx.expected_stage.as_deref().unwrap_or("scratch"))
        .expect("validated manifest");
    let mut out = FixtureOutcome {
        name: fx.name.clone(),
        stages,
        class,
        interpreter: interp_value,
        vm: vm_value,
        native,
        fresh_ir,
        mismatches,
        violations: Vec::new(),
        feature_tags: fx.feature_tags.clone(),
        expected_stage,
        expected_unsupported: fx.expected_status.as_deref() == Some("unsupported"),
    };
    out.violations = if full_pipeline {
        expectation_violations(fx, &out)
    } else {
        Vec::new()
    };
    out
}

/// Compare the actual outcome against the manifest's recorded expectation.
///
/// Manifest semantics: `expected_stage` is the first stage the fixture is *not*
/// fully expected to work at, and `expected_status` says whether it still works
/// everywhere (`success`) or is a known capability gap at exactly that stage
/// (`unsupported`). Stages before `expected_stage` must pass.
fn expectation_violations(fx: &FixtureEntry, out: &FixtureOutcome) -> Vec<Violation> {
    let mut v = Vec::new();
    let idx = |s: Stage| REPORT_STAGES.iter().position(|&x| x == s).unwrap();
    let expected_stage = Stage::from_str(fx.expected_stage.as_deref().unwrap_or("scratch"))
        .expect("validated manifest");
    let pos = idx(expected_stage);
    let success = fx.expected_status.as_deref() == Some("success");

    // Stages strictly before the boundary must pass.
    for &s in &REPORT_STAGES[..pos] {
        match out.stage_outcome(s).map(|o| o.outcome) {
            Some(Outcome::Pass) => {}
            Some(other) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(s),
                kind: "unexpected-block",
                message: format!("expected {s} to pass, recorded {other:?}"),
            }),
            None => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(s),
                kind: "not-measured",
                message: format!("expected {s} to pass, but it was not executed"),
            }),
        }
    }

    // Success fixtures: the boundary stage (scratch) must also pass.
    if success {
        match out.stage_outcome(Stage::Scratch).map(|o| o.outcome) {
            Some(Outcome::Pass) => {}
            Some(other) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Scratch),
                kind: "unexpected-block",
                message: format!("expected scratch to pass, recorded {other:?}"),
            }),
            None => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Scratch),
                kind: "not-measured",
                message: "expected scratch to pass, but it was not executed".to_string(),
            }),
        }
    }

    // Interpreter value must equal the expected result wherever the fixture is
    // expected to run on the interpreter (boundary beyond interpreter).
    let interpreter_expected = success || pos > idx(Stage::Interpreter);
    if interpreter_expected {
        if let (Some(exp), Some(iv)) = (fx.expected_result, out.interpreter.as_ref()) {
            if iv.to_u64() != Some(exp) {
                v.push(Violation {
                    fixture: fx.name.clone(),
                    stage: Some(Stage::Interpreter),
                    kind: "semantic-mismatch",
                    message: format!("interpreter returned {iv:?}, expected {exp}"),
                });
            }
        }
        if let Some(n) = &out.native {
            if let Some(exp) = fx.expected_result {
                if n.exit_masked != (exp & 0xFF) as u32 {
                    v.push(Violation {
                        fixture: fx.name.clone(),
                        stage: Some(Stage::Interpreter),
                        kind: "native-mismatch",
                        message: format!(
                            "native exit {} disagrees with expected {}",
                            n.exit_masked,
                            exp & 0xFF
                        ),
                    });
                }
            }
        }
    }

    // VM exact-value expectation (the `vm.exact` pin).
    if let Some(BackendExpect::Exact { exact }) = &fx.vm {
        match (out.stage_outcome(Stage::Vm).map(|o| o.outcome), out.vm.as_ref()) {
            (Some(Outcome::Pass), Some(vv)) => {
                if vv.to_u64() != Some(*exact) {
                    v.push(Violation {
                        fixture: fx.name.clone(),
                        stage: Some(Stage::Vm),
                        kind: "semantic-mismatch",
                        message: format!("vm returned {vv:?}, expected {exact}"),
                    });
                }
            }
            (Some(other), _) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Vm),
                kind: "unexpected-block",
                message: format!("expected vm exact {exact}, recorded {other:?}"),
            }),
            (None, _) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Vm),
                kind: "not-measured",
                message: format!("expected vm exact {exact}, vm not executed"),
            }),
        }
    }

    // Known-unsupported boundary: the boundary stage must be Unsupported, and
    // when a rejection diagnostic is pinned it must appear in the message.
    if !success {
        match out.stage_outcome(expected_stage).map(|o| o.outcome) {
            Some(Outcome::Unsupported) => {
                let pin = match expected_stage {
                    Stage::Vm => fx.vm.as_ref(),
                    Stage::Scratch => fx.scratch.as_ref(),
                    _ => None,
                };
                if let Some(BackendExpect::Rejected { rejected }) = pin {
                    let actual = out
                        .stage_outcome(expected_stage)
                        .and_then(|o| o.message.clone())
                        .unwrap_or_default();
                    if !actual.contains(rejected) {
                        v.push(Violation {
                            fixture: fx.name.clone(),
                            stage: Some(expected_stage),
                            kind: "diagnostic-mismatch",
                            message: format!("expected diagnostic containing {rejected:?}, got: {actual}"),
                        });
                    }
                }
            }
            Some(Outcome::Pass) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(expected_stage),
                kind: "missing-unsupported",
                message: format!("expected {expected_stage} to be unsupported, but it passed"),
            }),
            Some(Outcome::Fail) => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(expected_stage),
                kind: "unexpected-fail",
                message: format!(
                    "expected {expected_stage} unsupported, recorded Fail: {}",
                    out.stage_outcome(expected_stage)
                        .and_then(|o| o.message.clone())
                        .unwrap_or_default()
                ),
            }),
            None => v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(expected_stage),
                kind: "not-measured",
                message: format!("expected {expected_stage} unsupported, but it was not executed"),
            }),
        }
    }

    // Scratch construct expectation (constructs:true / constructs:false).
    if let Some(BackendExpect::Constructs { constructs }) = &fx.scratch {
        let actual_pass = matches!(
            out.stage_outcome(Stage::Scratch).map(|o| o.outcome),
            Some(Outcome::Pass)
        );
        if *constructs && !actual_pass {
            v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Scratch),
                kind: "unexpected-block",
                message: "expected scratch to construct, but it did not".to_string(),
            });
        }
        if !*constructs && actual_pass {
            v.push(Violation {
                fixture: fx.name.clone(),
                stage: Some(Stage::Scratch),
                kind: "unexpected-success",
                message: "expected scratch to reject, but it constructed".to_string(),
            });
        }
    }
    v
}

fn classify_llvm_error(e: &LlvmError) -> (Outcome, String) {
    match e {
        // A well-formed corpus fixture that the parser/translator cannot handle
        // is a capability gap, never a fixture bug.
        LlvmError::UnsupportedType(_)
        | LlvmError::UnsupportedInstruction(_)
        | LlvmError::UnsupportedIcmpPredicate(_)
        | LlvmError::Parse(_) => (Outcome::Unsupported, e.to_string()),
        LlvmError::UndefinedValue(_) | LlvmError::Translation(_) => {
            let msg = e.to_string().to_lowercase();
            let gap = ["unsupported", "not support", "cannot lower", "no lowering"]
                .iter()
                .any(|k| msg.contains(k));
            if gap {
                (Outcome::Unsupported, e.to_string())
            } else {
                (Outcome::Fail, e.to_string())
            }
        }
    }
}

fn classify_interp_error(e: &InterpError) -> (Outcome, String) {
    let msg = format!("{e:?}");
    match e {
        InterpError::UnsupportedInstruction(_) => (Outcome::Unsupported, msg),
        _ if msg.to_lowercase().contains("unsupported") => (Outcome::Unsupported, msg),
        _ => (Outcome::Fail, msg),
    }
}
