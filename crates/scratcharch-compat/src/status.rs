//! Pipeline-stage model and fixture result classification.
//!
//! The benchmark measures success *through the layered toolchain*, one stage at
//! a time:
//!
//! ```text
//! LLVM parser → SAIR (translate + validate) → optimizer → interpreter
//!                                                    ↘ ISA lowering → ISA VM
//!                                                    ↘ ScratchGraph backend
//! ```
//!
//! Every stage records a three-way outcome — [`Outcome::Pass`],
//! [`Outcome::Unsupported`], [`Outcome::Fail`] — which keeps the two very
//! different kinds of "did not work" apart:
//!
//! * **Unsupported** — a capability gap: the toolchain parsed a real LLVM
//!   construct and rejects it with a diagnostic that names the missing
//!   capability (`float`, a vector type, an `atomicrmw`, an `llvm.*`
//!   intrinsic…).
//! * **Fail** — a genuine error: validation rejected a module the translator
//!   produced, an interpreter/VM error class that is *not* an unsupported
//!   construct, a native differential that disagreed, etc.
//!
//! A fixture-level [`ResultClass`] collapses the first blocking stage into the
//! nine-value classification used for the report.

use std::fmt;

/// Canonical feature taxonomy used by the manifest and the feature breakdown.
pub const FEATURES: &[&str] = &[
    "integer",
    "i64",
    "bitwise",
    "shift",
    "memory",
    "global",
    "phi",
    "switch",
    "intrinsic",
    "pointer",
    "struct",
    "array",
    "aggregate-abi",
    "struct-param",
    "struct-return",
    "nested-aggregate",
    "float",
    "vector",
    "indirect-call",
    "atomic",
];

/// A measured pipeline stage. Ordered by the toolchain's front-to-back flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// LLVM text → parsed program (`scratcharch-llvm` parser).
    Parser,
    /// Parsed program → validated SAIR module (translator + `IrModule::validate`).
    Sair,
    /// SAIR optimization passes.
    Optimizer,
    /// SAIR interpreter execution (the semantic reference).
    Interpreter,
    /// ISA lowering → stack VM execution.
    Vm,
    /// ScratchGraph backend lowering (construction).
    Scratch,
}

impl Stage {
    /// Stable lowercase identifier (used in JSON, the CLI, and the manifest).
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Parser => "parser",
            Stage::Sair => "sair",
            Stage::Optimizer => "optimizer",
            Stage::Interpreter => "interpreter",
            Stage::Vm => "vm",
            Stage::Scratch => "scratch",
        }
    }

    /// Dashboard label.
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Parser => "Parser",
            Stage::Sair => "SAIR",
            Stage::Optimizer => "Optimizer",
            Stage::Interpreter => "Interpreter",
            Stage::Vm => "VM",
            Stage::Scratch => "Scratch",
        }
    }

    /// Parse a stable lowercase identifier into a stage, if it names one.
    /// (Named `from_str` returning `Option` for ergonomic use in the manifest
    /// and CLI; deliberately not the `std::str::FromStr` trait.)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Stage> {
        match s {
            "parser" => Some(Stage::Parser),
            "sair" => Some(Stage::Sair),
            "optimizer" => Some(Stage::Optimizer),
            "interpreter" => Some(Stage::Interpreter),
            "vm" => Some(Stage::Vm),
            "scratch" => Some(Stage::Scratch),
            _ => None,
        }
    }
}

/// The stages reported as dashboard axes. The optimizer is internal: it runs
/// between SAIR and the execution backends but has no dashboard row.
pub const REPORT_STAGES: [Stage; 5] = [
    Stage::Parser,
    Stage::Sair,
    Stage::Interpreter,
    Stage::Vm,
    Stage::Scratch,
];

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Three-way outcome recorded per stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The stage produced its correct result (or, for the Scratch backend,
    /// constructed a project).
    Pass,
    /// The stage ran but the construct is a capability gap the layer does not
    /// implement (never a silent fallback — always a named diagnostic).
    Unsupported,
    /// The stage failed with a genuine error (wrong result, validation
    /// rejection of our own output, unexpected runtime error, …).
    Fail,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Unsupported => "unsupported",
            Outcome::Fail => "fail",
        }
    }
}

/// Fixture-level result classification.
///
/// "Not implemented" and "implemented but wrong" are deliberately distinct:
/// every `*Unsupported` block that surfaces as one of the failure classes
/// below carries an [`Outcome::Unsupported`] stage record, while
/// [`ResultClass::SemanticMismatch`] is a pure correctness failure and must
/// never be conflated with a capability gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultClass {
    /// Blocked at the LLVM parser.
    ParseFailure,
    /// Blocked at LLVM→SAIR translation or SAIR validation.
    SairFailure,
    /// Blocked during SAIR optimization (expected to be vanishingly rare).
    OptimizationFailure,
    /// Blocked in the SAIR interpreter (error or unsupported construct).
    InterpreterFailure,
    /// Blocked at ISA lowering to the VM.
    LoweringFailure,
    /// Blocked during ISA VM execution.
    VmFailure,
    /// Blocked at SAIR→ScratchGraph lowering.
    ScratchBackendFailure,
    /// A supported construct produced the *wrong* result (interpreter/VM/native
    /// disagree with the expected value, or each other). Always a correctness
    /// failure.
    SemanticMismatch,
    /// The fixture ran end-to-end and matched expectations on every layer.
    Success,
}

impl ResultClass {
    /// Stable lowercase identifier for JSON and text reports.
    pub fn as_str(&self) -> &'static str {
        match self {
            ResultClass::ParseFailure => "parse-failure",
            ResultClass::SairFailure => "sair-failure",
            ResultClass::OptimizationFailure => "optimization-failure",
            ResultClass::InterpreterFailure => "interpreter-failure",
            ResultClass::LoweringFailure => "lowering-failure",
            ResultClass::VmFailure => "vm-failure",
            ResultClass::ScratchBackendFailure => "scratch-backend-failure",
            ResultClass::SemanticMismatch => "semantic-mismatch",
            ResultClass::Success => "success",
        }
    }

    /// The stage most directly responsible for a non-success class, if any.
    pub fn blocking_stage(&self) -> Option<Stage> {
        match self {
            ResultClass::ParseFailure => Some(Stage::Parser),
            ResultClass::SairFailure => Some(Stage::Sair),
            ResultClass::OptimizationFailure => Some(Stage::Optimizer),
            ResultClass::InterpreterFailure => Some(Stage::Interpreter),
            ResultClass::LoweringFailure | ResultClass::VmFailure => Some(Stage::Vm),
            ResultClass::ScratchBackendFailure => Some(Stage::Scratch),
            ResultClass::SemanticMismatch | ResultClass::Success => None,
        }
    }
}

impl fmt::Display for ResultClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_order_is_front_to_back() {
        assert!(Stage::Parser < Stage::Sair);
        assert!(Stage::Sair < Stage::Optimizer);
        assert!(Stage::Optimizer < Stage::Interpreter);
        assert!(Stage::Interpreter < Stage::Vm);
        assert!(Stage::Vm < Stage::Scratch);
    }

    #[test]
    fn report_stages_cover_dashboard() {
        assert_eq!(
            REPORT_STAGES.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            vec!["parser", "sair", "interpreter", "vm", "scratch"]
        );
    }

    #[test]
    fn stage_parse_round_trip() {
        for s in REPORT_STAGES {
            assert_eq!(Stage::from_str(s.as_str()), Some(s));
        }
        assert_eq!(Stage::from_str("optimizer"), Some(Stage::Optimizer));
        assert_eq!(Stage::from_str("bogus"), None);
    }

    #[test]
    fn feature_taxonomy_covers_spec() {
        // The manifest taxonomy must include every category the benchmark
        // reports on.
        for f in [
            "integer",
            "i64",
            "bitwise",
            "shift",
            "memory",
            "global",
            "phi",
            "switch",
            "intrinsic",
            "pointer",
            "struct",
            "array",
            "aggregate-abi",
            "struct-param",
            "struct-return",
            "nested-aggregate",
            "float",
            "vector",
            "indirect-call",
            "atomic",
        ] {
            assert!(FEATURES.contains(&f), "missing canonical feature {f}");
        }
    }

    #[test]
    fn outcome_and_class_names() {
        assert_eq!(Outcome::Pass.as_str(), "pass");
        assert_eq!(ResultClass::Success.as_str(), "success");
        assert_eq!(ResultClass::SemanticMismatch.as_str(), "semantic-mismatch");
        assert_eq!(
            ResultClass::VmFailure.blocking_stage(),
            Some(Stage::Vm)
        );
        assert_eq!(ResultClass::Success.blocking_stage(), None);
    }
}
