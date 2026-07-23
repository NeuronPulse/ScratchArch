//! Portable panic / abort / trap routines.
//!
//! These functions currently return runtime errors rather than halting the
//! machine. A future backend may choose to lower them to a hardware trap or to
//! an infinite loop, but the portable interface remains the same.

use crate::RuntimeError;

/// Abruptly terminate execution.
///
/// The portable implementation signals [`RuntimeError::Abort`]. A host that can
/// halt cleanly may handle this error as a fatal exit.
pub fn abort() -> Result<(), RuntimeError> {
    Err(RuntimeError::Abort)
}

/// Report a fatal logic error.
///
/// The portable implementation signals [`RuntimeError::Panic`] carrying the
/// supplied message. This matches the intent of `panic` in higher-level
/// languages without assuming a particular unwind mechanism.
pub fn panic(msg: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::Panic {
        msg: Some(msg.to_string()),
    })
}

/// Execute a debug / breakpoint trap.
///
/// The portable implementation signals [`RuntimeError::Trap`]. A backend may
/// lower this to a software breakpoint or to a no-op if trapping is unavailable.
pub fn trap() -> Result<(), RuntimeError> {
    Err(RuntimeError::Trap)
}
