use scratcharch_core::instruction::Instruction as IsaInstr;
use scratcharch_core::program::{Program, Function};
use crate::instruction::Instruction as SairInstr;
use crate::instruction::{GepIndex, Terminator};
use crate::r#module::IrModule;
use crate::types::IrType;
use crate::value::Constant;

#[derive(Debug)]
pub enum LowerError {
    UnsupportedType(String),
    UnsupportedInstruction(String),
    PhiNotSupported,
    MultiBlockNotSupported(String),
    ValidationError(String),
    GepNotSupported(String),
}

pub struct IsaLowerer;

impl Default for IsaLowerer {
    fn default() -> Self {
        Self::new()
    }
}

impl IsaLowerer {
    pub fn new() -> Self {
        IsaLowerer
    }

    pub fn lower(&self, module: &IrModule) -> Result<Program, LowerError> {
        module.validate().map_err(LowerError::ValidationError)?;
        let mut program = Program::new(&module.entry);

        for func in &module.functions {
            if func.blocks.len() > 1 {
                return Err(LowerError::MultiBlockNotSupported(func.name.clone()));
            }
            if has_phi(func) {
                return Err(LowerError::PhiNotSupported);
            }
            if has_gep(func) {
                return Err(LowerError::GepNotSupported(func.name.clone()));
            }
            let lowered = self.lower_function(func)?;
            program.add_function(lowered);
        }
        Ok(program)
    }

    fn lower_function(&self, func: &crate::function::IrFunction) -> Result<Function, LowerError> {
        let mut f = Function::new(&func.name);
        let block = &func.blocks[0];

        for instr in &block.instructions {
            self.push_instr(instr, &mut f)?;
        }

        self.push_term(&block.terminator, &mut f);
        Ok(f)
    }

    fn push_instr(&self, instr: &SairInstr, f: &mut Function) -> Result<(), LowerError> {
        match instr {
            SairInstr::Add { .. } => f.push(None::<&str>, IsaInstr::I32Add),
            SairInstr::Sub { .. } => f.push(None::<&str>, IsaInstr::I32Sub),
            SairInstr::Mul { .. } => f.push(None::<&str>, IsaInstr::I32Mul),
            SairInstr::Div { .. } => f.push(None::<&str>, IsaInstr::I32Div),
            SairInstr::Rem { .. } => f.push(None::<&str>, IsaInstr::I32Rem),
            SairInstr::Eq { .. } => f.push(None::<&str>, IsaInstr::Eq),
            SairInstr::Lt { .. } => f.push(None::<&str>, IsaInstr::Lt),
            SairInstr::Gt { .. } => f.push(None::<&str>, IsaInstr::Gt),
            SairInstr::Const(c) => match c {
                Constant::I32(v) => f.push(None::<&str>, IsaInstr::ConstI32(*v)),
                Constant::F64(v) => f.push(None::<&str>, IsaInstr::ConstF64(*v)),
                Constant::I1(v) => f.push(None::<&str>, IsaInstr::ConstI32(if *v { 1 } else { 0 })),
                Constant::I8(v) => f.push(None::<&str>, IsaInstr::ConstI32(*v as u32)),
                Constant::I16(v) => f.push(None::<&str>, IsaInstr::ConstI32(*v as u32)),
            },
            SairInstr::Alloca { ty, count } => {
                f.push(None::<&str>, IsaInstr::ConstI32(ty.size_in_bytes() * count));
                f.push(None::<&str>, IsaInstr::Alloc);
            }
            SairInstr::Load { .. } => f.push(None::<&str>, IsaInstr::Load),
            SairInstr::Store { .. } => f.push(None::<&str>, IsaInstr::Store),
            SairInstr::Call { callee, .. } => {
                f.push(None::<&str>, IsaInstr::Call(callee.clone()));
            }
            SairInstr::Phi { .. } => return Err(LowerError::PhiNotSupported),
            SairInstr::Gep { elem_ty, indices, .. } => {
                self.lower_gep(elem_ty, indices, f);
            }
        }
        Ok(())
    }

    fn push_term(&self, term: &Terminator, f: &mut Function) {
        match term {
            Terminator::Branch { target } => f.push(None::<&str>, IsaInstr::Jump(target.clone())),
            Terminator::CondBranch { true_target, false_target, .. } => {
                f.push(None::<&str>, IsaInstr::Branch(true_target.clone(), false_target.clone()));
            }
            Terminator::Return { .. } => f.push(None::<&str>, IsaInstr::Return),
        }
    }

    fn lower_gep(&self, elem_ty: &IrType, indices: &[GepIndex], f: &mut Function) {
        for idx in indices {
            match idx {
                GepIndex::Dynamic(_) => {
                    f.push(None::<&str>, IsaInstr::ConstI32(elem_ty.size_in_bytes()));
                    f.push(None::<&str>, IsaInstr::I32Mul);
                }
                GepIndex::StructField(field) => {
                    f.push(None::<&str>, IsaInstr::ConstI32(field * 4));
                }
            }
        }
        let count = indices.len() as u32;
        for _ in 0..count.saturating_sub(1) {
            f.push(None::<&str>, IsaInstr::I32Add);
        }
    }
}

fn has_phi(func: &crate::function::IrFunction) -> bool {
    func.blocks.iter().any(|b| b.instructions.iter().any(|i| matches!(i, SairInstr::Phi { .. })))
}

fn has_gep(func: &crate::function::IrFunction) -> bool {
    func.blocks.iter().any(|b| b.instructions.iter().any(|i| matches!(i, SairInstr::Gep { .. })))
}
