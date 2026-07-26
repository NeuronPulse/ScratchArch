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
pub mod json_exporter;
pub mod lower;
pub mod runtime;

pub use exporter::{Infallible, ScratchExporter};
pub use ir::{
    Broadcast, EventHat, Expr, List, ListScope, Procedure, ProcedureParam, ProcedurePrototype,
    Project, Script, ScriptEntry, Sprite, Stage, Stmt, StopOption, Value, Variable, VariableScope,
};
pub use json_exporter::JsonExporter;
pub use lower::{LowerError, ScratchGraphLowerer};
pub use runtime::{
    EventState, ExecutionStatus, GlobalState, ProgramCounter, RuntimeState, SchedulerState,
    ThreadContext, ThreadId, ThreadState, WaitReason,
};
