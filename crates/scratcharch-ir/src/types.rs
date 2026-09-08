use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    I1,
    I8,
    I16,
    I32,
    I64,
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
            IrType::I64 => 8,
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
            IrType::I64 => 8,
            IrType::F64 => 8,
            IrType::Pointer => 4,
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self, IrType::Void)
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64
        )
    }

    /// Bit width of an integer type, `None` for non-integers.
    pub fn integer_width(&self) -> Option<u32> {
        match self {
            IrType::I1 => Some(1),
            IrType::I8 => Some(8),
            IrType::I16 => Some(16),
            IrType::I32 => Some(32),
            IrType::I64 => Some(64),
            _ => None,
        }
    }

    /// The scalar types that a `ptr` may be cast to and from (SA48 pointers are
    /// 32-bit cells).
    pub fn is_pointer_sized_integer(&self) -> bool {
        matches!(self, IrType::I32 | IrType::I64)
    }

    pub fn to_core_type(&self) -> Option<scratcharch_core::types::Type> {
        match self {
            IrType::I1 => Some(scratcharch_core::types::Type::I1),
            IrType::I8 => Some(scratcharch_core::types::Type::I8),
            IrType::I16 => Some(scratcharch_core::types::Type::I16),
            IrType::I32 => Some(scratcharch_core::types::Type::I32),
            // SA48 cells are 48 bits wide; a 64-bit integer needs two cells and
            // has no single-cell core representation. It stays SAIR-only and the
            // ISA lowerer reports an explicit multi-cell diagnostic.
            IrType::I64 => None,
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
            IrType::I64 => write!(f, "i64"),
            IrType::F64 => write!(f, "f64"),
            IrType::Pointer => write!(f, "ptr"),
            IrType::Void => write!(f, "void"),
        }
    }
}
