use crate::types::IrType;
use crate::value::{Constant, ValueId};

pub type BlockLabel = String;
pub type FuncName = String;

#[derive(Debug, Clone)]
pub enum GepIndex {
    Dynamic(ValueId),
    StructField(u32),
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Add {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Sub {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Mul {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Div {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Rem {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Eq {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Lt {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Gt {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Const(Constant),
    Alloca {
        ty: IrType,
        count: u32,
    },
    Load {
        ty: IrType,
        addr: ValueId,
    },
    Store {
        ty: IrType,
        value: ValueId,
        addr: ValueId,
    },
    Call {
        return_ty: IrType,
        callee: FuncName,
        args: Vec<ValueId>,
    },
    Phi {
        ty: IrType,
        incoming: Vec<(ValueId, BlockLabel)>,
    },
    Gep {
        result_ty: IrType,
        elem_ty: IrType,
        base: ValueId,
        indices: Vec<GepIndex>,
    },
}

impl Instruction {
    pub fn result_type(&self) -> Option<IrType> {
        match self {
            Instruction::Const(c) => Some(c.ty()),
            Instruction::Add { ty, .. }
            | Instruction::Sub { ty, .. }
            | Instruction::Mul { ty, .. }
            | Instruction::Div { ty, .. }
            | Instruction::Rem { ty, .. } => Some(*ty),
            Instruction::Eq { .. }
            | Instruction::Lt { .. }
            | Instruction::Gt { .. } => Some(IrType::I1),
            Instruction::Alloca { .. } => Some(IrType::Pointer),
            Instruction::Load { ty, .. } => Some(*ty),
            Instruction::Store { .. } => None,
            Instruction::Call { return_ty, .. } => {
                if return_ty.is_void() { None } else { Some(*return_ty) }
            }
            Instruction::Phi { ty, .. } => Some(*ty),
            Instruction::Gep { result_ty, .. } => Some(*result_ty),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Branch {
        target: BlockLabel,
    },
    CondBranch {
        condition: ValueId,
        true_target: BlockLabel,
        false_target: BlockLabel,
    },
    Return {
        value: Option<ValueId>,
    },
}
