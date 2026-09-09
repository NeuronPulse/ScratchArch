//! ScratchArch LLVM Compatibility Benchmark — external measurement layer.
//!
//! `scratcharch-compat` is deliberately **not** part of any frontend or backend
//! crate. It sits on top of the existing toolchain and asks the same layered
//! question for every fixture in a committed corpus:
//!
//! ```text
//! .ll (or fresh .c → clang) → LLVM parser → SAIR → optimizer → interpreter
//!                                                       ↘ ISA lowering → ISA VM
//!                                                       ↘ ScratchGraph backend
//! ```
//!
//! Each stage records PASS / FAIL / UNSUPPORTED; a fixture-level class collapses
//! the first blocking stage; the manifest doubles as a regression oracle. See
//! the sibling modules and `docs/design/LLVM_COMPATIBILITY_BENCHMARK.md`.
//!
//! Crate layout:
//!
//! * [`progress`] — the one shared character-progress-bar helper.
//! * [`status`] — stage model, three-way outcome, fixture result classes,
//!   canonical feature taxonomy.
//! * [`manifest`] — the corpus manifest schema + validation.
//! * [`native`] — native differential execution of committed `.c` sources.
//! * [`runner`] — the staged pipeline runner and result assembly.
//! * [`report`] — metrics, dashboard text, machine-readable JSON.

pub mod manifest;
pub mod native;
pub mod progress;
pub mod report;
pub mod runner;
pub mod status;

pub use manifest::{BackendExpect, FixtureEntry, Manifest};
pub use progress::{format_progress_bar, format_progress_bar_ascii, percent};
pub use report::{build_report, render_json, render_text, Report};
pub use runner::{Corpus, EngineValue, FixtureOutcome, RunConfig};
pub use status::{Outcome, ResultClass, Stage};
