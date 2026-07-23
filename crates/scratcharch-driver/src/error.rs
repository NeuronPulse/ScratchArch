use std::fmt;

/// Errors that can occur while driving the ScratchArch compilation pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum DriverError {
    /// LLVM IR parsing or translation failed.
    Llvm(String),
    /// The translated SAIR module failed validation.
    Validation(String),
    /// Interpretation failed.
    Execution(String),
    /// VM load or execution failed.
    Vm(String),
    /// An I/O operation failed.
    Io(String),
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriverError::Llvm(msg) => write!(f, "llvm error: {}", msg),
            DriverError::Validation(msg) => write!(f, "validation error: {}", msg),
            DriverError::Execution(msg) => write!(f, "execution error: {}", msg),
            DriverError::Vm(msg) => write!(f, "vm error: {}", msg),
            DriverError::Io(msg) => write!(f, "io error: {}", msg),
        }
    }
}

impl std::error::Error for DriverError {}

impl From<std::io::Error> for DriverError {
    fn from(err: std::io::Error) -> Self {
        DriverError::Io(err.to_string())
    }
}
