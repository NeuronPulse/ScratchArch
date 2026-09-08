use crate::block::BasicBlock;
use crate::debug::DebugLoc;
use crate::function::IrFunction;
use crate::instruction::{CastOp, GepIndex, Instruction, Terminator};
use crate::r#module::IrModule;
use crate::types::IrType;
use crate::value::{Constant, ValueId};

pub struct IrBuilder {
    pub module: IrModule,
    current_func: Option<String>,
    current_block: Option<String>,
}

impl IrBuilder {
    pub fn new(entry: impl Into<String>) -> Self {
        IrBuilder {
            module: IrModule::new(entry),
            current_func: None,
            current_block: None,
        }
    }

    pub fn start_function(&mut self, name: impl Into<String>, return_ty: IrType) {
        let func = IrFunction::new(name, return_ty);
        let name_str = func.name.clone();
        self.module.add_function(func);
        self.current_func = Some(name_str);
        self.current_block = None;
    }

    pub fn add_param(&mut self, ty: IrType, name: impl Into<String>) -> ValueId {
        let func = self.module.get_function_mut(self.current_func.as_ref().unwrap()).unwrap();
        func.add_param(ty, name)
    }

    pub fn new_block(&mut self, label: impl Into<String>) {
        let block = BasicBlock::new(label);
        let label_str = block.label.clone();
        let func = self.module.get_function_mut(self.current_func.as_ref().unwrap()).unwrap();
        func.add_block(block);
        self.current_block = Some(label_str);
    }

    pub fn new_value(&mut self, ty: IrType, name: Option<impl Into<String>>) -> ValueId {
        let func = self.module.get_function_mut(self.current_func.as_ref().unwrap()).unwrap();
        func.new_value(ty, name)
    }

    fn current_block_mut(&mut self) -> &mut BasicBlock {
        let func_label = self.current_func.clone().unwrap();
        let block_label = self.current_block.clone().unwrap();
        let func = self.module.get_function_mut(&func_label).unwrap();
        let idx = func.block_index(&block_label).unwrap();
        &mut func.blocks[idx]
    }

    pub fn emit(&mut self, instr: Instruction) {
        self.current_block_mut().push(instr);
    }

    fn emit_value(&mut self, instr: Instruction, ty: IrType, name: Option<impl Into<String>>) -> ValueId {
        let id = self.new_value(ty, name);
        self.emit(instr);
        id
    }

    pub fn add(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Add { ty, lhs, rhs }, ty, None::<&str>)
    }

    pub fn sub(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Sub { ty, lhs, rhs }, ty, None::<&str>)
    }

    pub fn mul(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Mul { ty, lhs, rhs }, ty, None::<&str>)
    }

    pub fn div(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Div { ty, lhs, rhs }, ty, None::<&str>)
    }

    pub fn rem(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Rem { ty, lhs, rhs }, ty, None::<&str>)
    }

    pub fn eq(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Eq { ty, lhs, rhs }, IrType::I1, None::<&str>)
    }

    pub fn lt(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Lt { ty, lhs, rhs }, IrType::I1, None::<&str>)
    }

    pub fn gt(&mut self, ty: IrType, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit_value(Instruction::Gt { ty, lhs, rhs }, IrType::I1, None::<&str>)
    }

    pub fn const_i32(&mut self, val: u32) -> ValueId {
        self.emit_value(Instruction::Const(Constant::I32(val)), IrType::I32, None::<&str>)
    }

    pub fn const_f64(&mut self, val: f64) -> ValueId {
        self.emit_value(Instruction::Const(Constant::F64(val)), IrType::F64, None::<&str>)
    }

    pub fn const_i1(&mut self, val: bool) -> ValueId {
        self.emit_value(Instruction::Const(Constant::I1(val)), IrType::I1, None::<&str>)
    }

    pub fn const_i8(&mut self, val: u8) -> ValueId {
        self.emit_value(Instruction::Const(Constant::I8(val)), IrType::I8, None::<&str>)
    }

    pub fn const_i16(&mut self, val: u16) -> ValueId {
        self.emit_value(Instruction::Const(Constant::I16(val)), IrType::I16, None::<&str>)
    }

    pub fn const_i64(&mut self, val: u64) -> ValueId {
        self.emit_value(Instruction::Const(Constant::I64(val)), IrType::I64, None::<&str>)
    }

    /// Emit an LLVM conversion as `Instruction::Cast`. `from_ty` is the static
    /// type of `value`; the produced value has static type `to_ty`.
    pub fn cast(
        &mut self,
        op: CastOp,
        from_ty: IrType,
        to_ty: IrType,
        value: ValueId,
    ) -> ValueId {
        self.emit_value(
            Instruction::Cast { op, from_ty, to_ty, value },
            to_ty,
            None::<&str>,
        )
    }

    /// `select ty cond a b` — `a` when `cond` (i1) is true, else `b`.
    pub fn select(
        &mut self,
        ty: IrType,
        condition: ValueId,
        then_value: ValueId,
        else_value: ValueId,
    ) -> ValueId {
        self.emit_value(
            Instruction::Select { ty, condition, then_value, else_value },
            ty,
            None::<&str>,
        )
    }

    /// Terminate the current block with LLVM `unreachable`.
    pub fn unreachable(&mut self) {
        self.set_terminator(Terminator::Unreachable);
    }

    pub fn gep(&mut self, elem_ty: IrType, base: ValueId, indices: Vec<GepIndex>) -> ValueId {
        self.emit_value(Instruction::Gep {
            result_ty: IrType::Pointer,
            elem_ty,
            base,
            indices,
        }, IrType::Pointer, None::<&str>)
    }

    pub fn alloca(&mut self, ty: IrType) -> ValueId {
        self.emit_value(Instruction::Alloca { ty, count: 1 }, IrType::Pointer, None::<&str>)
    }

    pub fn alloca_array(&mut self, ty: IrType, count: u32) -> ValueId {
        self.emit_value(Instruction::Alloca { ty, count }, IrType::Pointer, None::<&str>)
    }

    pub fn load(&mut self, ty: IrType, addr: ValueId) -> ValueId {
        self.emit_value(Instruction::Load { ty, addr }, ty, None::<&str>)
    }

    pub fn store(&mut self, ty: IrType, value: ValueId, addr: ValueId) {
        self.emit(Instruction::Store { ty, value, addr });
    }

    pub fn call(&mut self, return_ty: IrType, callee: impl Into<String>, args: Vec<ValueId>) -> Option<ValueId> {
        let callee_str: String = callee.into();
        let id = if !return_ty.is_void() {
            Some(self.new_value(return_ty, None::<&str>))
        } else {
            None
        };
        self.emit(Instruction::Call {
            return_ty,
            callee: callee_str,
            args,
        });
        id
    }

    pub fn phi(&mut self, ty: IrType, incoming: Vec<(ValueId, impl Into<String>)>) -> ValueId {
        let incoming = incoming.into_iter()
            .map(|(v, l)| (v, l.into()))
            .collect();
        self.emit_value(Instruction::Phi { ty, incoming }, ty, None::<&str>)
    }

    pub fn set_terminator(&mut self, term: Terminator) {
        self.current_block_mut().set_terminator(term);
    }

    pub fn ret(&mut self, value: Option<ValueId>) {
        self.set_terminator(Terminator::Return { value });
    }

    pub fn br(&mut self, target: impl Into<String>) {
        self.set_terminator(Terminator::Branch { target: target.into() });
    }

    pub fn cond_br(&mut self, condition: ValueId, true_target: impl Into<String>, false_target: impl Into<String>) {
        self.set_terminator(Terminator::CondBranch {
            condition,
            true_target: true_target.into(),
            false_target: false_target.into(),
        });
    }

    pub fn finish(mut self) -> IrModule {
        if let Some(func_name) = self.current_func.take() {
            let _ = func_name;
        }
        self.module
    }

    pub fn set_entry_block(&mut self, label: impl Into<String>) {
        if let Some(func_name) = &self.current_func {
            if let Some(func) = self.module.get_function_mut(func_name) {
                func.entry_block = label.into();
            }
        }
    }

    /// Set the debug location on the most recently emitted instruction in the
    /// current block. Does nothing if no block is active.
    pub fn set_last_debug_loc(&mut self, debug: Option<DebugLoc>) {
        if let (Some(func_name), Some(block_label)) = (&self.current_func, &self.current_block) {
            if let Some(func) = self.module.get_function_mut(func_name) {
                if let Some(idx) = func.block_index(block_label) {
                    let block = &mut func.blocks[idx];
                    if !block.instructions.is_empty() {
                        block.set_debug_loc(block.instructions.len() - 1, debug);
                    }
                }
            }
        }
    }

    /// Update an operand of an existing phi instruction.
    ///
    /// This is useful for loop-carried phis whose back-edge value is defined
    /// after the phi itself, a common situation in SSA construction.
    pub fn set_phi_operand(
        &mut self,
        block_label: &str,
        instr_idx: usize,
        pred_label: &str,
        value: ValueId,
    ) {
        let func_name = self.current_func.as_ref().expect("no current function");
        let func = self.module.get_function_mut(func_name).expect("function missing");
        let block_idx = func.block_index(block_label).expect("block not found");
        let instr = func.blocks[block_idx]
            .instructions
            .get_mut(instr_idx)
            .expect("instruction index out of bounds");
        if let Instruction::Phi { incoming, .. } = instr {
            for (v, l) in incoming.iter_mut() {
                if l == pred_label {
                    *v = value;
                    return;
                }
            }
        }
        panic!("phi operand for predecessor '{}' not found", pred_label);
    }
}
