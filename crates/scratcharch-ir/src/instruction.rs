use crate::types::IrType;
use crate::value::{Constant, ValueId};

pub type BlockLabel = String;
pub type FuncName = String;

#[derive(Debug, Clone)]
pub enum GepIndex {
    Dynamic(ValueId),
    StructField(u32),
}

/// The LLVM integer/pointer conversions that SAIR models directly.
///
/// SA48 keeps 32-bit pointers, so `PtrToInt`/`IntToPtr` only change static
/// type (the value stays a 32-bit integer / pointer). `Zext`/`Sext`/`Trunc`
/// change bit width with LLVM semantics; `Bitcast` keeps the bits and only
/// reinterprets the static type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastOp {
    Zext,
    Sext,
    Trunc,
    Bitcast,
    PtrToInt,
    IntToPtr,
}

impl CastOp {
    pub fn name(&self) -> &'static str {
        match self {
            CastOp::Zext => "zext",
            CastOp::Sext => "sext",
            CastOp::Trunc => "trunc",
            CastOp::Bitcast => "bitcast",
            CastOp::PtrToInt => "ptrtoint",
            CastOp::IntToPtr => "inttoptr",
        }
    }
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
    And {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Or {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Xor {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Shl {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Lshr {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Ashr {
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
    /// Bit-width-preserving or bit-width-changing conversion. `value` has static
    /// type `from_ty`; the produced SSA value has static type `to_ty`.
    Cast {
        op: CastOp,
        from_ty: IrType,
        to_ty: IrType,
        value: ValueId,
    },
    /// LLVM `select i1 cond, a, b`: yields `a` when cond is true, else `b`.
    Select {
        ty: IrType,
        condition: ValueId,
        then_value: ValueId,
        else_value: ValueId,
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
            | Instruction::Rem { ty, .. }
            | Instruction::And { ty, .. }
            | Instruction::Or { ty, .. }
            | Instruction::Xor { ty, .. }
            | Instruction::Shl { ty, .. }
            | Instruction::Lshr { ty, .. }
            | Instruction::Ashr { ty, .. } => Some(*ty),
            Instruction::Eq { .. }
            | Instruction::Lt { .. }
            | Instruction::Gt { .. } => Some(IrType::I1),
            Instruction::Cast { to_ty, .. } => Some(*to_ty),
            Instruction::Select { ty, .. } => Some(*ty),
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
    /// LLVM `unreachable`: control must never reach here. A block ending in
    /// `Unreachable` is a legal predecessor in the CFG (LLVM may branch to it)
    /// but never produces a runtime edge. The interpreter treats reaching it as
    /// an error rather than UB.
    Unreachable,
}

impl Terminator {
    /// Blocks this terminator may pass control to.
    pub fn referenced_labels(&self) -> Vec<BlockLabel> {
        match self {
            Terminator::Branch { target } => vec![target.clone()],
            Terminator::CondBranch {
                true_target,
                false_target,
                ..
            } => vec![true_target.clone(), false_target.clone()],
            Terminator::Return { .. } | Terminator::Unreachable => Vec::new(),
        }
    }
}
