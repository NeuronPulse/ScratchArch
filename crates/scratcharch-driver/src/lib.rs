//! # ScratchArch Driver
//!
//! A unified compilation pipeline for ScratchArch:
//!
//! ```text
//! LLVM IR text → LLVM translator → SAIR module → optimization passes → interpreter
//! ```
//!
//! The driver is intentionally backend-agnostic at the API level: only the
//! interpreter backend is wired up in v0.1 because the VM backend still supports
//! only single-block functions.

pub mod config;
pub mod driver;
pub mod error;

pub use config::{CompileConfig, OptLevel};
pub use driver::{CompileDriver, CompiledModule, ExecutionValue};
pub use error::DriverError;
