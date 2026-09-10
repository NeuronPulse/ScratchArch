//! # ScratchArch Runtime Library (SART) v0.1
//!
//! Portable, architecture-independent runtime routines for ScratchArch.
//!
//! The runtime is intentionally decoupled from the ISA, LLVM, Scratch, and any
//! particular backend. It operates only through the [`ByteMemory`] trait so that
//! frontends written for C, Rust, Zig, or other languages can reuse the same
//! routines.

use std::fmt;

pub mod memory;
pub mod string;
pub mod intrinsics;
pub mod panic;
pub mod bitops;

pub use bitops::{bit_intrinsic_value, BitIntrinsicKind};
pub use intrinsics::{dispatch_intrinsic, IntrinsicFn, IntrinsicRegistry, IntrinsicSignature};
pub use memory::{memcmp, memcpy, memset, memmove};
pub use panic::{abort, panic, trap};
pub use string::{strcmp, strcpy, strlen, strncpy};

#[cfg(test)]
mod tests;

/// Errors that can be returned by runtime routines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// A memory access fell outside the available address space.
    MemoryOutOfBounds { addr: u32 },
    /// A string exceeded the available memory without finding a null terminator.
    UnterminatedString { addr: u32 },
    /// `abort` was called.
    Abort,
    /// `panic` was called, optionally with a message.
    Panic { msg: Option<String> },
    /// `trap` was called.
    Trap,
    /// An unknown intrinsic name was dispatched.
    UnknownIntrinsic(String),
    /// An intrinsic was called with the wrong number of arguments.
    BadIntrinsicArgs { name: String, expected: usize, got: usize },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::MemoryOutOfBounds { addr } => {
                write!(f, "memory access out of bounds at address {addr:#x}")
            }
            RuntimeError::UnterminatedString { addr } => {
                write!(f, "unterminated string at address {addr:#x}")
            }
            RuntimeError::Abort => write!(f, "abort"),
            RuntimeError::Panic { msg: Some(m) } => write!(f, "panic: {m}"),
            RuntimeError::Panic { msg: None } => write!(f, "panic"),
            RuntimeError::Trap => write!(f, "trap"),
            RuntimeError::UnknownIntrinsic(n) => write!(f, "unknown intrinsic '{n}'"),
            RuntimeError::BadIntrinsicArgs { name, expected, got } => {
                write!(f, "intrinsic '{name}' expects {expected} arguments, got {got}")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

/// A runtime value returned by an intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// Unsigned 32-bit integer result.
    U32(u32),
    /// Signed 32-bit integer result.
    I32(i32),
    /// Pointer result.
    Pointer(u32),
}

/// Result of invoking an intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicResult {
    /// The intrinsic has no meaningful return value.
    Void,
    /// The intrinsic produced a value.
    Value(Value),
}

/// Architecture-independent byte-level memory interface.
///
/// All runtime routines operate through this trait. Implementations must perform
/// their own bounds checking; out-of-bounds accesses produce
/// [`RuntimeError::MemoryOutOfBounds`].
pub trait ByteMemory {
    /// Read a single byte from `addr`.
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError>;

    /// Write a single byte to `addr`.
    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError>;
}

impl ByteMemory for Vec<u8> {
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError> {
        self.get(addr as usize)
            .copied()
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })
    }

    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError> {
        let slot = self
            .get_mut(addr as usize)
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })?;
        *slot = value;
        Ok(())
    }
}

impl ByteMemory for &mut [u8] {
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError> {
        self.get(addr as usize)
            .copied()
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })
    }

    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError> {
        let slot = self
            .get_mut(addr as usize)
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })?;
        *slot = value;
        Ok(())
    }
}
