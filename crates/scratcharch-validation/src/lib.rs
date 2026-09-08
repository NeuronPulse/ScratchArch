//! Roundtrip and semantic-validation framework for ScratchGraph.
//!
//! This crate composes the Scratch toolchain crates that otherwise do not
//! depend on each other — [`scratcharch_scratchgraph`] (IR, parser, semantic
//! normalizer), [`scratcharch_analyzer`] (semantic diff), [`scratcharch_sb3`]
//! (SB3 archive reader/writer) and [`scratcharch_transform`] (optimization
//! passes) — into the *validation engine* behind the roundtrip test matrix, the
//! transform-preservation checks, and the `scratcharch verify` CLI subcommand.
//!
//! The contract being verified is defined in
//! `docs/specification/SCRATCH_SEMANTICS.md`: a conversion `X → Y → X'` is
//! *semantically preserving* when the normalized forms `N(X)` and `N(X')` are
//! equal (equivalently, when [`scratcharch_analyzer::semantic_diff`] reports no
//! differences). Equality is IR-level — never byte equality of `project.json`.

pub mod graph;
pub mod preservation;
pub mod roundtrip;
pub mod verify;

pub use graph::validate as validate_graph;
pub use preservation::{
    constant_folding_reaches_canonical_fold, dce_preserves_liveness,
    empty_block_removal_matches, variable_analysis_keeps_referenced,
};
pub use roundtrip::{roundtrip_diff, roundtrip_sb3};
pub use verify::{verify_project, VerifyReport, PassCheck};
