use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    I1,
    I8,
    I16,
    I32,
    F64,
    Pointer,
}

impl Type {
    pub fn size_in_bytes(&self) -> u32 {
        match self {
            Type::I1 => 1,
            Type::I8 => 1,
            Type::I16 => 2,
            Type::I32 => 4,
            Type::F64 => 8,
            Type::Pointer => 4,
        }
    }

    pub fn alignment(&self) -> u32 {
        match self {
            Type::I1 => 1,
            Type::I8 => 1,
            Type::I16 => 2,
            Type::I32 => 4,
            Type::F64 => 8,
            Type::Pointer => 4,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I1 => write!(f, "i1"),
            Type::I8 => write!(f, "i8"),
            Type::I16 => write!(f, "i16"),
            Type::I32 => write!(f, "i32"),
            Type::F64 => write!(f, "f64"),
            Type::Pointer => write!(f, "ptr"),
        }
    }
}
