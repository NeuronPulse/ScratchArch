//! Roundtrip and semantic-validation framework for ScratchGraph.
//!
//! This crate composes the Scratch toolchain crates that otherwise do not
//! depend on each other — [`scratcharch_scratchgraph`] (IR, parser, semantic
//! normalizer), [`scratcharch_analyzer`] (semantic diff) and
//! [`scratcharch_sb3`] (SB3 archive reader/writer) — into the *validation
//! engine* behind the SB3 roundtrip test matrix.
//!
//! The contract being verified is defined in
//! `docs/specification/SCRATCH_SEMANTICS.md`: a conversion `X → Y → X'` is
//! *semantically preserving* when the normalized forms `N(X)` and `N(X')` are
//! equal (equivalently, when [`scratcharch_analyzer::semantic_diff`] reports no
//! differences). Equality is IR-level — never byte equality of `project.json`.

pub mod graph;
pub mod roundtrip;

pub use graph::validate as validate_graph;
pub use roundtrip::{roundtrip_diff, roundtrip_sb3};
