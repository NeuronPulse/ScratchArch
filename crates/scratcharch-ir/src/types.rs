use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    I1,
    I8,
    I16,
    I32,
    F64,
    Pointer,
    Void,
}

impl IrType {
    pub fn size_in_bytes(&self) -> u32 {
        match self {
            IrType::Void => 0,
            IrType::I1 => 1,
            IrType::I8 => 1,
            IrType::I16 => 2,
            IrType::I32 => 4,
            IrType::F64 => 8,
            IrType::Pointer => 4,
        }
    }

    pub fn alignment(&self) -> u32 {
        match self {
            IrType::Void => 1,
            IrType::I1 => 1,
            IrType::I8 => 1,
            IrType::I16 => 2,
            IrType::I32 => 4,
            IrType::F64 => 8,
            IrType::Pointer => 4,
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self, IrType::Void)
    }

    pub fn to_core_type(&self) -> Option<scratcharch_core::types::Type> {
        match self {
            IrType::I1 => Some(scratcharch_core::types::Type::I1),
            IrType::I8 => Some(scratcharch_core::types::Type::I8),
            IrType::I16 => Some(scratcharch_core::types::Type::I16),
            IrType::I32 => Some(scratcharch_core::types::Type::I32),
            IrType::F64 => Some(scratcharch_core::types::Type::F64),
            IrType::Pointer => Some(scratcharch_core::types::Type::Pointer),
            IrType::Void => None,
        }
    }
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrType::I1 => write!(f, "i1"),
            IrType::I8 => write!(f, "i8"),
            IrType::I16 => write!(f, "i16"),
            IrType::I32 => write!(f, "i32"),
            IrType::F64 => write!(f, "f64"),
            IrType::Pointer => write!(f, "ptr"),
            IrType::Void => write!(f, "void"),
        }
    }
}
