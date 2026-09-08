use crate::types::IrType;

pub type ValueId = usize;

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    I64(u64),
    I32(u32),
    F64(f64),
    I1(bool),
    I8(u8),
    I16(u16),
}

impl Constant {
    pub fn ty(&self) -> IrType {
        match self {
            Constant::I64(_) => IrType::I64,
            Constant::I1(_) => IrType::I1,
            Constant::I8(_) => IrType::I8,
            Constant::I16(_) => IrType::I16,
            Constant::I32(_) => IrType::I32,
            Constant::F64(_) => IrType::F64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SsaValue {
    pub id: ValueId,
    pub ty: IrType,
    pub name: Option<String>,
}

impl SsaValue {
    pub fn new(id: ValueId, ty: IrType, name: Option<String>) -> Self {
        SsaValue { id, ty, name }
    }

    pub fn display_name(&self) -> String {
        match &self.name {
            Some(n) => format!("%{n}"),
            None => format!("%{}", self.id),
        }
    }
}
