//! ScratchArch Scratch backend: ScratchGraph IR and exporters.
//!
//! This crate provides a Scratch-specific backend layer without coupling
//! Scratch serialization formats to the compiler core. The pipeline is:
//!
//! ```text
//! SAIR → ScratchGraph IR → ScratchExporter → project.json / sb3 / sprite3
//! ```
//!
//! ScratchGraph is a semantic intermediate representation of Scratch programs:
//! sprites, scripts, procedures, variables, and control-flow structures. It is
//! intentionally independent of `project.json` so that future format changes
//! only affect exporters.

pub mod exporter;
pub mod ir;
pub mod lower;

pub use exporter::{Infallible, ScratchExporter};
pub use ir::{
    Broadcast, Expr, Hat, List, Procedure, ProcedureParam, ProcedurePrototype, Project, Script,
    Sprite, Stage, Stmt, StopOption, Value, Variable,
};
pub use lower::{LowerError, ScratchGraphLowerer};
