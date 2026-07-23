//! # ScratchArch Driver
//!
//! A unified compilation pipeline for ScratchArch:
//!
//! ```text
//! LLVM IR text → LLVM translator → SAIR module → optimization passes → interpreter
//!                                                                    ↘ VM (lower to ISA)
//! ```
//!
//! The driver is backend-agnostic at the API level: the interpreter is the
//! default, and the VM backend can be selected via `CompileConfig::backend`.

pub mod config;
pub mod driver;
pub mod error;

pub use config::{CompileConfig, ExecutionBackend, OptLevel};
pub use driver::{CompileDriver, CompiledModule, ExecutionValue};
pub use error::DriverError;
