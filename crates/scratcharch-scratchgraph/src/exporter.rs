//! Exporter abstraction for ScratchGraph projects.
//!
//! ScratchGraph is serialization-agnostic. Implementations of `ScratchExporter`
//! translate a semantic `Project` into a concrete output format such as
//! Scratch 3 JSON, an sb3 archive, or scratchblocks text.

use crate::ir::Project;

/// Trait for ScratchGraph exporters.
///
/// Implementations must be deterministic and self-contained: the only input
/// is the `Project`. Any required IDs, counters, or format-specific state are
/// managed internally by the exporter.
pub trait ScratchExporter {
    /// Output type produced by the exporter (e.g. `String`, `serde_json::Value`).
    type Output;
    /// Error type returned on export failure.
    type Error: core::fmt::Display;

    /// Export a ScratchGraph project.
    fn export(&self, project: &Project) -> Result<Self::Output, Self::Error>;
}

/// Error type for exporters that never fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Infallible;

impl core::fmt::Display for Infallible {
    fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unreachable!("infallible exporter error")
    }
}
