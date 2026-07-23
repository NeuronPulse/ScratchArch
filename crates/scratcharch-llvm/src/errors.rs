use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LlvmError {
    Parse(String),
    UnsupportedType(String),
    UnsupportedInstruction(String),
    UnsupportedIcmpPredicate(String),
    UndefinedValue(String),
    Translation(String),
}

impl fmt::Display for LlvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlvmError::Parse(msg) => write!(f, "parse error: {}", msg),
            LlvmError::UnsupportedType(msg) => write!(f, "unsupported type: {}", msg),
            LlvmError::UnsupportedInstruction(msg) => write!(f, "unsupported instruction: {}", msg),
            LlvmError::UnsupportedIcmpPredicate(msg) => write!(f, "unsupported icmp predicate: {}", msg),
            LlvmError::UndefinedValue(msg) => write!(f, "undefined value: {}", msg),
            LlvmError::Translation(msg) => write!(f, "translation error: {}", msg),
        }
    }
}
