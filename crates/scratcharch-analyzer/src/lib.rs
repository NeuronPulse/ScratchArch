//! Static analysis tools for ScratchGraph programs.
//!
//! The analyzer operates on the Scratch-specific semantic IR provided by
//! `scratcharch-scratchgraph`. It does not depend on SAIR, the core ISA, or the
//! VM. This keeps Scratch-specific analysis logic isolated from the rest of the
//! compiler stack.
//!
//! Provided analyses:
//!
//! - `cfg`: build a control-flow graph over a sequence of ScratchGraph statements.
//! - `callgraph`: build a call graph of procedures and detect recursion.
//! - `reachability`: find scripts that cannot be reached from any event hat.
//! - `variables`: compute variable read/write usage.

pub mod cache;
pub mod callgraph;
pub mod cfg;
pub mod diff;
pub mod dot;
pub mod metrics;
pub mod reachability;
pub mod report;
pub mod variables;

pub use cache::{AnalysisCache, ProjectHash};
pub use callgraph::{CallGraph, CallGraphAnalysis, RecursionKind};
pub use cfg::{CfgAnalysis, CfgEdge, CfgNode, ControlFlowGraph};
pub use diff::{
    semantic_diff, semantic_diff_normalized, DiffCategory, DiffEntry, DiffFormat, DiffResult,
};
pub use dot::DotOutput;
pub use metrics::ComplexityMetrics;
pub use reachability::{ReachabilityAnalysis, UnreachableScript};
pub use report::{AnalysisReport, Report};
pub use variables::{VariableUsage, VariableUsageAnalysis, VariableUsageAnalyzer};
