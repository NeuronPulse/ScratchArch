use core::fmt;
use crate::types::Type;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    I1(bool),
    I8(u8),
    I16(u16),
    I32(u32),
    F64(f64),
    Pointer(u32),
}

impl Value {
    pub fn ty(&self) -> Type {
        match self {
            Value::I1(_) => Type::I1,
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::F64(_) => Type::F64,
            Value::Pointer(_) => Type::Pointer,
        }
    }

    pub fn as_i32(&self) -> Option<u32> {
        match self {
            Value::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i1(&self) -> Option<bool> {
        match self {
            Value::I1(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::F64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_address(&self) -> Option<u32> {
        match self {
            Value::Pointer(v) => Some(*v),
            Value::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn i32_wrapping_add(a: u32, b: u32) -> u32 {
        a.wrapping_add(b)
    }

    pub fn i32_wrapping_sub(a: u32, b: u32) -> u32 {
        a.wrapping_sub(b)
    }

    pub fn i32_wrapping_mul(a: u32, b: u32) -> u32 {
        a.wrapping_mul(b)
    }

    pub fn i32_div(a: u32, b: u32) -> Option<u32> {
        if b == 0 { None } else { Some(a / b) }
    }

    pub fn i32_rem(a: u32, b: u32) -> Option<u32> {
        if b == 0 { None } else { Some(a % b) }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::I1(v) => write!(f, "i1 {}", if *v { 1 } else { 0 }),
            Value::I8(v) => write!(f, "i8 {}", v),
            Value::I16(v) => write!(f, "i16 {}", v),
            Value::I32(v) => write!(f, "i32 {}", v),
            Value::F64(v) => write!(f, "f64 {}", v),
            Value::Pointer(v) => write!(f, "ptr 0x{v:x}"),
        }
    }
}
