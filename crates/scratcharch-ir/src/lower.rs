use std::collections::{HashMap, HashSet};

use scratcharch_core::instruction::Instruction as IsaInstr;
use scratcharch_core::program::{Function, Program};
use scratcharch_target::profile::TargetProfile;

use crate::block::BasicBlock;
use crate::function::IrFunction;
use crate::instruction::{CastOp, GepIndex, Instruction as SairInstr, Terminator};
use crate::r#module::IrModule;
use crate::types::IrType;
use crate::value::{Constant, ValueId};

#[derive(Debug)]
pub enum LowerError {
    UnsupportedType(String),
    UnsupportedInstruction(String),
    ValidationError(String),
}

pub struct IsaLowerer {
    profile: TargetProfile,
}

impl Default for IsaLowerer {
    fn default() -> Self {
        Self::new()
    }
}

impl IsaLowerer {
    pub fn new() -> Self {
        IsaLowerer {
            profile: TargetProfile::sa48(),
        }
    }

    pub fn with_profile(profile: TargetProfile) -> Self {
        IsaLowerer { profile }
    }

    pub fn lower(&self, module: &IrModule) -> Result<Program, LowerError> {
        module.validate().map_err(LowerError::ValidationError)?;
        let mut program = Program::new(&module.entry);

        for func in &module.functions {
            let lowered = self.lower_function(func)?;
            program.add_function(lowered);
        }

        // Shift instructions lower to calls into software shift helpers (the ISA
        // only has 32-bit word shifts; a width-aware shift over one or two limbs
        // is a small loop). Append exactly the helpers the module references.
        for name in SHIFT_HELPERS {
            if module_uses_shift(module, name) {
                program.add_function(build_shift_helper(name));
            }
        }

        // Two-limb multiply and division have no ISA widening op, so they lower
        // to calls into software helpers built from the existing 32-bit-word
        // primitives (`I32Mul`, shifts, compares) — see `build_mul_helper` and
        // `build_udivrem_helper`. The helpers are appended only when the module
        // actually references the corresponding SAIR op at 64-bit width.
        if module_uses_wide_mul(module) {
            program.add_function(build_mul_helper());
        }
        if module_uses_wide_divrem(module) {
            program.add_function(build_udivrem_helper());
        }
        Ok(program)
    }

    fn lower_function(&self, func: &IrFunction) -> Result<Function, LowerError> {
        // 1. Compute the result id for every result-producing instruction so we
        //    can map emitted instructions back to their SSA value slot.
        let mut next_id = func.params.len();
        let mut result_ids: HashMap<(String, usize), ValueId> = HashMap::new();
        for block in &func.blocks {
            for (idx, instr) in block.instructions.iter().enumerate() {
                if instr.result_type().is_some() {
                    result_ids.insert((block.label.clone(), idx), next_id);
                    next_id += 1;
                }
            }
        }

        // 2. Build lowering context and working copy for CFG transformations.
        let ctx = LowerCtx::new(&self.profile, func)?;
        let mut working = func.clone();
        let trampoline_labels = split_critical_edges(&mut working);

        // 3. Collect phi copies per edge and decide where each copy sequence
        //    must be emitted (start of a block or end of a block).
        let edge_copies = collect_phi_copies(&working, &ctx)?;
        let mut start_copies: HashMap<String, Vec<Copy>> = HashMap::new();
        let mut end_copies: HashMap<String, Vec<Copy>> = HashMap::new();
        for ((pred_label, succ_label), copies) in edge_copies {
            let placement = copy_placement(&working, &trampoline_labels, &pred_label, &succ_label);
            match placement {
                CopyPlacement::AtStartOf(block) => {
                    start_copies.entry(block).or_default().extend(copies);
                }
                CopyPlacement::AtEndOf(block) => {
                    end_copies.entry(block).or_default().extend(copies);
                }
            }
        }

        // 4. Emit the function.
        let mut f = Function::new(&func.name);
        f.param_cells = ctx.param_cells;
        f.local_count = ctx.local_count;
        f.return_cells = ctx.return_cells;

        // Prologue: pop parameter cells from the operand stack into slots.
        for slot in (0..ctx.param_cells).rev() {
            f.push(None::<&str>, IsaInstr::LocalSet(slot));
        }

        let mut emitter = FuncEmitter::new(&mut f);
        let mut select_counter: usize = 0;
        for block in &working.blocks {
            emitter.set_label(&block.label);

            // Emit incoming phi copies (including trampoline blocks).
            if let Some(copies) = start_copies.get(&block.label) {
                emit_copies(copies, ctx.temp_slot, &mut emitter);
            }

            // Lower non-phi instructions.
            for (idx, instr) in block.instructions.iter().enumerate() {
                if matches!(instr, SairInstr::Phi { .. }) {
                    continue;
                }
                let result_id = result_ids.get(&(block.label.clone(), idx)).copied();
                self.lower_instr(instr, result_id, &ctx, &mut emitter, &mut select_counter)?;
            }

            // Emit outgoing phi copies before the terminator.
            if let Some(copies) = end_copies.get(&block.label) {
                emit_copies(copies, ctx.temp_slot, &mut emitter);
            }

            self.lower_term(&block.terminator, &ctx, &mut emitter)?;
        }

        Ok(f)
    }

    fn lower_instr(
        &self,
        instr: &SairInstr,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
        select_counter: &mut usize,
    ) -> Result<(), LowerError> {
        match instr {
            SairInstr::Add { ty, lhs, rhs }
            | SairInstr::Sub { ty, lhs, rhs }
            | SairInstr::Mul { ty, lhs, rhs }
            | SairInstr::Div { ty, lhs, rhs }
            | SairInstr::Rem { ty, lhs, rhs } => {
                let limbs = self.vm_limbs(*ty)?;
                if limbs == 1 {
                    self.emit_load(*lhs, ctx, f)?;
                    self.emit_load(*rhs, ctx, f)?;
                    let isa_op = match instr {
                        SairInstr::Add { .. } => IsaInstr::I32Add,
                        SairInstr::Sub { .. } => IsaInstr::I32Sub,
                        SairInstr::Mul { .. } => IsaInstr::I32Mul,
                        SairInstr::Div { .. } => IsaInstr::I32Div,
                        SairInstr::Rem { .. } => IsaInstr::I32Rem,
                        _ => unreachable!(),
                    };
                    f.push(isa_op);
                    if let Some(id) = result_id {
                        self.emit_store(id, ctx, f)?;
                    }
                } else {
                    match instr {
                        SairInstr::Add { .. } => self.lower_wide_add(*lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Sub { .. } => self.lower_wide_sub(*lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Mul { .. } => self.lower_wide_mul(*lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Div { .. } => self.lower_wide_divrem(DivRemKind::Div, *lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Rem { .. } => self.lower_wide_divrem(DivRemKind::Rem, *lhs, *rhs, result_id, ctx, f)?,
                        _ => unreachable!(),
                    }
                }
            }
            SairInstr::And { ty, lhs, rhs }
            | SairInstr::Or { ty, lhs, rhs }
            | SairInstr::Xor { ty, lhs, rhs }
            | SairInstr::Shl { ty, lhs, rhs }
            | SairInstr::Lshr { ty, lhs, rhs }
            | SairInstr::Ashr { ty, lhs, rhs } => {
                let limbs = self.vm_limbs(*ty)?;
                if limbs == 1 {
                    match instr {
                        SairInstr::Shl { .. } => self.lower_shift_single(*ty, ShiftKind::Shl, *lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Lshr { .. } => self.lower_shift_single(*ty, ShiftKind::Lshr, *lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Ashr { .. } => self.lower_shift_single(*ty, ShiftKind::Ashr, *lhs, *rhs, result_id, ctx, f)?,
                        _ => {
                            let isa_op = match instr {
                                SairInstr::And { .. } => IsaInstr::And,
                                SairInstr::Or { .. } => IsaInstr::Or,
                                SairInstr::Xor { .. } => IsaInstr::Xor,
                                _ => unreachable!(),
                            };
                            self.emit_cast_source_as_i32(*ty, *lhs, ctx, f)?;
                            self.emit_cast_source_as_i32(*ty, *rhs, ctx, f)?;
                            f.push(isa_op);
                            if let Some(id) = result_id {
                                self.store_masked_single(*ty, id, ctx, f)?;
                            }
                        }
                    }
                } else {
                    match instr {
                        SairInstr::Shl { .. } => {
                            self.lower_shift_wide(ShiftKind::Shl, *lhs, *rhs, result_id, ctx, f)?
                        }
                        SairInstr::Lshr { .. } => {
                            self.lower_shift_wide(ShiftKind::Lshr, *lhs, *rhs, result_id, ctx, f)?
                        }
                        SairInstr::Ashr { .. } => {
                            self.lower_shift_wide(ShiftKind::Ashr, *lhs, *rhs, result_id, ctx, f)?
                        }
                        _ => {
                            let isa_op = match instr {
                                SairInstr::And { .. } => IsaInstr::And,
                                SairInstr::Or { .. } => IsaInstr::Or,
                                SairInstr::Xor { .. } => IsaInstr::Xor,
                                _ => unreachable!(),
                            };
                            self.lower_wide_bitwise(*ty, *lhs, *rhs, result_id, isa_op, ctx, f)?;
                        }
                    }
                }
            }
            SairInstr::Eq { ty, lhs, rhs }
            | SairInstr::Lt { ty, lhs, rhs }
            | SairInstr::Gt { ty, lhs, rhs } => {
                let limbs = self.vm_limbs(*ty)?;
                if limbs == 1 {
                    self.emit_load(*lhs, ctx, f)?;
                    self.emit_load(*rhs, ctx, f)?;
                    let isa_op = match instr {
                        SairInstr::Eq { .. } => IsaInstr::Eq,
                        SairInstr::Lt { .. } => IsaInstr::Lt,
                        SairInstr::Gt { .. } => IsaInstr::Gt,
                        _ => unreachable!(),
                    };
                    f.push(isa_op);
                    if let Some(id) = result_id {
                        self.emit_store(id, ctx, f)?;
                    }
                } else {
                    match instr {
                        SairInstr::Eq { .. } => self.lower_wide_eq(*lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Lt { .. } => self.lower_wide_cmp_lt(*lhs, *rhs, result_id, ctx, f)?,
                        SairInstr::Gt { .. } => self.lower_wide_cmp_gt(*lhs, *rhs, result_id, ctx, f)?,
                        _ => unreachable!(),
                    }
                }
            }
            SairInstr::Const(c) => {
                self.emit_const(c, f)?;
                if let Some(id) = result_id {
                    self.emit_store(id, ctx, f)?;
                }
            }
            SairInstr::Alloca { ty, count } => {
                f.push(IsaInstr::ConstI32(ty.size_in_bytes() * count));
                f.push(IsaInstr::Alloc);
                if let Some(id) = result_id {
                    self.emit_store(id, ctx, f)?;
                }
            }
            SairInstr::Load { ty, addr } => {
                let limbs = self.vm_limbs(*ty)?;
                if limbs == 1 {
                    self.lower_load_single(*ty, *addr, result_id, ctx, f)?;
                } else {
                    self.lower_wide_load(*addr, result_id, ctx, f)?;
                }
            }
            SairInstr::Store { ty, value, addr } => {
                let limbs = self.vm_limbs(*ty)?;
                if limbs == 1 {
                    self.lower_store_single(*ty, *value, *addr, ctx, f)?;
                } else {
                    self.lower_wide_store(*value, *addr, ctx, f)?;
                }
            }
            SairInstr::Call {
                return_ty,
                callee,
                args,
            } => {
                for arg in args {
                    self.emit_load(*arg, ctx, f)?;
                }
                f.push(IsaInstr::Call(callee.clone()));
                if !return_ty.is_void() {
                    if let Some(id) = result_id {
                        self.emit_store(id, ctx, f)?;
                    }
                }
            }
            SairInstr::Gep {
                elem_ty,
                base,
                indices,
                ..
            } => {
                self.lower_gep(*base, elem_ty, indices, ctx, f)?;
                if let Some(id) = result_id {
                    self.emit_store(id, ctx, f)?;
                }
            }
            SairInstr::Cast { op, from_ty, to_ty, value } => {
                self.lower_cast(*op, *from_ty, *to_ty, *value, result_id, ctx, f, select_counter)?;
            }
            SairInstr::Select {
                ty,
                condition,
                then_value,
                else_value,
            } => {
                self.lower_select(*ty, *condition, *then_value, *else_value, result_id, ctx, f, select_counter)?;
            }
            SairInstr::Phi { .. } => {
                // Phis are lowered via edge copies; the instruction itself is a no-op.
            }
        }
        Ok(())
    }

    fn lower_term(
        &self,
        term: &Terminator,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        match term {
            Terminator::Branch { target } => {
                f.push(IsaInstr::Jump(target.clone()));
            }
            Terminator::CondBranch {
                condition,
                true_target,
                false_target,
            } => {
                self.emit_load(*condition, ctx, f)
                    .expect("cond branch condition value missing");
                f.push(IsaInstr::Branch(true_target.clone(), false_target.clone()));
            }
            Terminator::Return { value } => {
                if let Some(v) = value {
                    self.emit_load(*v, ctx, f).expect("return value missing");
                }
                f.push(IsaInstr::Return);
            }
            Terminator::Unreachable => {
                // LLVM `unreachable` is a trap. SAIR declares the path
                // impossible, so the realized ISA marks reaching it as a
                // terminal, program-declared stop via the `Trap` primitive
                // (ISA.md Appendix A, EXECUTION_MODEL.md §5.6). It is not a
                // fall-through, a silent return, or a machine error.
                f.push(IsaInstr::Trap);
            }
        }
        Ok(())
    }

    fn emit_load(&self, value: ValueId, ctx: &LowerCtx, f: &mut FuncEmitter<'_>) -> Result<(), LowerError> {
        let cells = ctx.cells(value)?;
        let first = ctx.first_slot(value)?;
        for offset in 0..cells {
            f.push(IsaInstr::LocalGet(first + offset));
        }
        Ok(())
    }

    fn emit_store(&self, value: ValueId, ctx: &LowerCtx, f: &mut FuncEmitter<'_>) -> Result<(), LowerError> {
        let cells = ctx.cells(value)?;
        let first = ctx.first_slot(value)?;
        for offset in (0..cells).rev() {
            f.push(IsaInstr::LocalSet(first + offset));
        }
        Ok(())
    }

    fn emit_const(&self, c: &Constant, f: &mut FuncEmitter<'_>) -> Result<(), LowerError> {
        match c {
            Constant::I32(v) => f.push(IsaInstr::ConstI32(*v)),
            Constant::F64(v) => f.push(IsaInstr::ConstF64(*v)),
            Constant::I1(v) => f.push(IsaInstr::ConstI1(*v)),
            Constant::I8(v) => f.push(IsaInstr::ConstI32(*v as u32)),
            Constant::I16(v) => f.push(IsaInstr::ConstI32(*v as u32)),
            Constant::I64(v) => {
                // A 64-bit integer constant becomes two 32-bit limbs (low first,
                // so that the high limb is on top after the two pushes and the
                // symmetric `emit_store` drops it into the high slot first).
                self.push_i64_limbs(*v, f)?;
            }
        }
        Ok(())
    }

    /// Push the two 32-bit limbs of an `i64` bit pattern onto the operand stack
    /// (low limb first). Errors when the target profile does not decompose a
    /// 64-bit integer into exactly two VM words.
    fn push_i64_limbs(&self, v: u64, f: &mut FuncEmitter<'_>) -> Result<(), LowerError> {
        let cells = LowerCtx::cell_count_for_type(&self.profile, &IrType::I64);
        if cells != 2 {
            return Err(LowerError::UnsupportedType(format!(
                "i64 constant is {cells} profile cells on {}; the VM decomposes 64-bit integers into exactly two 32-bit limbs",
                self.profile.name,
            )));
        }
        f.push(IsaInstr::ConstI32(v as u32));
        f.push(IsaInstr::ConstI32((v >> 32) as u32));
        Ok(())
    }

    fn lower_gep(
        &self,
        base: ValueId,
        elem_ty: &IrType,
        indices: &[GepIndex],
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        self.emit_load(base, ctx, f)?;
        for idx in indices {
            match idx {
                GepIndex::Dynamic(value) => {
                    // The byte offset arrives as a full-width integer (the LLVM
                    // frontend builds it in i64 for any dynamic index). Only its
                    // low 32 bits can address the VM's 32-bit pointer space, so a
                    // multi-cell index contributes its low limb; the profile's
                    // wrap semantics make this the exact low bits of the offset.
                    let cells = ctx.cells(*value)?;
                    if cells > 1 {
                        let first = ctx.first_slot(*value)?;
                        f.push(IsaInstr::LocalGet(first));
                    } else {
                        self.emit_load(*value, ctx, f)?;
                    }
                    f.push(IsaInstr::ConstI32(elem_ty.size_in_bytes()));
                    f.push(IsaInstr::I32Mul);
                    f.push(IsaInstr::I32Add);
                }
                GepIndex::StructField(_) => {
                    return Err(LowerError::UnsupportedType(
                        "struct-field GEP offsets require type layout".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Number of operand-stack words (limbs) an integer `ty` occupies on the VM.
    ///
    /// The VM stores every integer cell as a 32-bit word. An i1/i8/i16/i32 value
    /// is one profile cell on the SA48/sa32 profiles and so one limb; an i64 is
    /// two cells there and is carried as two 32-bit limbs. A profile that would
    /// need a different number of cells — or a 64-bit integer in a single cell —
    /// has no faithful decomposition into 32-bit VM words and is reported rather
    /// than approximated.
    fn vm_limbs(&self, ty: IrType) -> Result<u32, LowerError> {
        let cells = LowerCtx::cell_count_for_type(&self.profile, &ty);
        if cells == 1 {
            if let Some(w) = ty.integer_width() {
                if w > 32 {
                    return Err(LowerError::UnsupportedType(format!(
                        "integer {ty} ({w} bits) would need one {}-bit profile cell but the VM words are 32 bits",
                        self.profile.cell_width,
                    )));
                }
            }
            return Ok(1);
        }
        match ty {
            IrType::I64 if cells == 2 => Ok(2),
            _ => Err(LowerError::UnsupportedType(format!(
                "multi-cell integer {ty} is {cells} profile cells on {}; the VM can only carry a 64-bit integer as two 32-bit limbs",
                self.profile.name,
            ))),
        }
    }

    /// Push one 32-bit limb of `value` onto the operand stack.
    fn emit_load_limb(
        &self,
        value: ValueId,
        limb: u32,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let first = ctx.first_slot(value)?;
        f.push(IsaInstr::LocalGet(first + limb));
        Ok(())
    }

    /// Lower an SAIR `select` to an if/else over branches, since the ISA has no
    /// select/CMOV instruction. The condition selects between pushing the
    /// `then` cells or the `else` cells onto the operand stack; both arms merge
    /// at a `join` label where the winner is stored into the result slot. The
    /// expansion is cell-count generic, so it also carries multi-cell (i64)
    /// selects — each arm simply pushes both of the value's limbs.
    ///
    /// The synthesized blocks are pure ISA-level structure (fresh labels, no
    /// SAIR block), emitted inline inside the enclosing block, so they do not
    /// participate in phi copies or critical-edge splitting.
    ///
    /// The `&mut usize` counter threads the codebase's `lower_*` helper style
    /// (op operands + ctx + emitter), which exceeds clippy's arity default.
    #[allow(clippy::too_many_arguments)]
    fn lower_select(
        &self,
        ty: IrType,
        cond: ValueId,
        then_value: ValueId,
        else_value: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
        counter: &mut usize,
    ) -> Result<(), LowerError> {
        let _ = ty;
        let Some(_id) = result_id else {
            // A select with no result is dead code (no side effects): skip it
            // rather than emit unbalanced branches.
            return Ok(());
        };

        let n = *counter;
        *counter += 1;
        let t_label = format!("__select_{n}_then");
        let e_label = format!("__select_{n}_else");
        let j_label = format!("__select_{n}_join");

        self.emit_load(cond, ctx, f)?;
        f.push(IsaInstr::Branch(t_label.clone(), e_label.clone()));

        f.set_label(&t_label);
        self.emit_load(then_value, ctx, f)?;
        f.push(IsaInstr::Jump(j_label.clone()));

        f.set_label(&e_label);
        self.emit_load(else_value, ctx, f)?;

        f.set_label(&j_label);
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    // ---- Multi-cell (i64) integer lowering -------------------------------
    //
    // A 64-bit integer occupies two adjacent value slots: slot `first` holds the
    // low 32-bit limb and slot `first + 1` the high limb (little-endian order,
    // matching both the slot convention of `emit_load`/`emit_store` and the byte
    // order a two-word memory read produces). Each helper computes limb results
    // and stores them as soon as they are ready, keeping the operand stack
    // shallow — the ISA has no rotate/reorder instruction beyond `Pick`.

    /// Slot index of `limb` (0 = low) of `value`.
    fn slot_of_limb(&self, value: ValueId, limb: u32, ctx: &LowerCtx) -> Result<u32, LowerError> {
        Ok(ctx.first_slot(value)? + limb)
    }

    fn lower_wide_add(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        let l0 = self.slot_of_limb(lhs, 0, ctx)?;
        let r0 = self.slot_of_limb(rhs, 0, ctx)?;
        let q = ctx.first_slot(id)?;

        // s0 = (l0 + r0) mod 2^32; carry-out of the low limb = s0 <u l0.
        f.push(IsaInstr::LocalGet(l0));
        f.push(IsaInstr::LocalGet(r0));
        f.push(IsaInstr::I32Add);
        f.push(IsaInstr::LocalSet(q)); // low limb result
        // carry = low sum wrapped (unsigned)
        f.push(IsaInstr::LocalGet(q));
        f.push(IsaInstr::LocalGet(l0));
        f.push(IsaInstr::Lt);
        // s1 = (l1 + r1) mod 2^32, then add the carry (an i1 reads as 0/1).
        f.push(IsaInstr::LocalGet(l0 + 1));
        f.push(IsaInstr::LocalGet(r0 + 1));
        f.push(IsaInstr::I32Add);
        f.push(IsaInstr::I32Add);
        f.push(IsaInstr::LocalSet(q + 1));
        Ok(())
    }

    fn lower_wide_sub(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        let l0 = self.slot_of_limb(lhs, 0, ctx)?;
        let r0 = self.slot_of_limb(rhs, 0, ctx)?;
        let q = ctx.first_slot(id)?;

        // d0 = (l0 - r0) mod 2^32.
        f.push(IsaInstr::LocalGet(l0));
        f.push(IsaInstr::LocalGet(r0));
        f.push(IsaInstr::I32Sub);
        f.push(IsaInstr::LocalSet(q));
        // t1 = (l1 - r1) mod 2^32, then subtract the borrow = l0 <u r0.
        f.push(IsaInstr::LocalGet(l0 + 1));
        f.push(IsaInstr::LocalGet(r0 + 1));
        f.push(IsaInstr::I32Sub);
        // Recompute borrow from the untouched operand slots: t1 - borrow.
        f.push(IsaInstr::LocalGet(l0));
        f.push(IsaInstr::LocalGet(r0));
        f.push(IsaInstr::Lt);
        f.push(IsaInstr::I32Sub);
        f.push(IsaInstr::LocalSet(q + 1));
        Ok(())
    }

    /// Canonicalize the single cell on top of the operand stack to a plain I32
    /// word of the same bit content. An `I1` flag becomes `0`/`1` (so boolean
    /// results can flow through the ISA's integer `And`/`Or`); a `Pointer`
    /// carrier becomes the equivalent `I32` address (so a reinterpreted pointer
    /// can reach the strict `i32` word ops unchanged). An `I32` is untouched.
    fn canonicalize_top_word(&self, f: &mut FuncEmitter<'_>) {
        f.push(IsaInstr::ConstI32(0));
        f.push(IsaInstr::I32Add);
    }

    /// Reduce the 0/1 I32 word on top of the stack to a true i1 flag.
    fn reduce_top_to_flag(&self, f: &mut FuncEmitter<'_>) {
        f.push(IsaInstr::ConstI32(1));
        f.push(IsaInstr::Eq);
    }

    fn lower_wide_eq(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let l0 = self.slot_of_limb(lhs, 0, ctx)?;
        let r0 = self.slot_of_limb(rhs, 0, ctx)?;
        // a == b  <=>  (l0 ^ r0) | (l1 ^ r1) == 0.
        f.push(IsaInstr::LocalGet(l0));
        f.push(IsaInstr::LocalGet(r0));
        f.push(IsaInstr::Xor);
        f.push(IsaInstr::LocalGet(l0 + 1));
        f.push(IsaInstr::LocalGet(r0 + 1));
        f.push(IsaInstr::Xor);
        f.push(IsaInstr::Or);
        f.push(IsaInstr::ConstI32(0));
        f.push(IsaInstr::Eq);
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    fn lower_wide_cmp_lt(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let l0 = self.slot_of_limb(lhs, 0, ctx)?;
        let r0 = self.slot_of_limb(rhs, 0, ctx)?;
        // l <u r  <=>  (l1 <u r1) | ((l1 == r1) & (l0 <u r0))
        self.push_limb_cmp(l0, r0, 1, IsaInstr::Lt, f);
        self.push_limb_cmp(l0, r0, 1, IsaInstr::Eq, f);
        self.push_limb_cmp(l0, r0, 0, IsaInstr::Lt, f);
        f.push(IsaInstr::And);
        f.push(IsaInstr::Or);
        self.reduce_top_to_flag(f);
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    fn lower_wide_cmp_gt(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let l0 = self.slot_of_limb(lhs, 0, ctx)?;
        let r0 = self.slot_of_limb(rhs, 0, ctx)?;
        // l >u r  <=>  (l1 >u r1) | ((l1 == r1) & (l0 >u r0))
        self.push_limb_cmp(l0, r0, 1, IsaInstr::Gt, f);
        self.push_limb_cmp(l0, r0, 1, IsaInstr::Eq, f);
        self.push_limb_cmp(l0, r0, 0, IsaInstr::Gt, f);
        f.push(IsaInstr::And);
        f.push(IsaInstr::Or);
        self.reduce_top_to_flag(f);
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    /// Emit one limb comparison `slot_a op slot_b` widened to an I32 0/1 word.
    fn push_limb_cmp(
        &self,
        slot_a: u32,
        slot_b: u32,
        limb: u32,
        op: IsaInstr,
        f: &mut FuncEmitter<'_>,
    ) {
        f.push(IsaInstr::LocalGet(slot_a + limb));
        f.push(IsaInstr::LocalGet(slot_b + limb));
        f.push(op);
        self.canonicalize_top_word(f);
    }

    /// Load an i64 from `addr`: low limb at `addr`, high limb at `addr + 4`.
    fn lower_wide_load(
        &self,
        addr: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        self.emit_load(addr, ctx, f)?;
        f.push(IsaInstr::Load); // low limb
        self.emit_load(addr, ctx, f)?;
        f.push(IsaInstr::ConstI32(4));
        f.push(IsaInstr::I32Add);
        f.push(IsaInstr::Load); // high limb at addr + 4
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    /// Store an i64 to `addr`: low limb at `addr`, high limb at `addr + 4`.
    fn lower_wide_store(
        &self,
        value: ValueId,
        addr: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let v0 = self.slot_of_limb(value, 0, ctx)?;
        self.emit_load(addr, ctx, f)?;
        f.push(IsaInstr::LocalGet(v0));
        f.push(IsaInstr::Store);
        self.emit_load(addr, ctx, f)?;
        f.push(IsaInstr::ConstI32(4));
        f.push(IsaInstr::I32Add);
        f.push(IsaInstr::LocalGet(v0 + 1));
        f.push(IsaInstr::Store);
        Ok(())
    }

    // ---- Width-accurate single-limb memory ---------------------------------
    //
    // A single-limb type (i1/i8/i16/i32/ptr) occupies one profile cell, but its
    // *memory* width is `size_in_bytes()` bytes, not the VM word's 4. `Load` and
    // `Store` must therefore write exactly that many bytes so neighbouring bytes
    // are never clobbered or misread (little-endian, matching the interpreter's
    // `size_in_bytes`-exact reads/writes). Word-width types (i32/ptr) keep the
    // word `Load`/`Store`; sub-word types use `Load8`/`Store8`. Alignment is
    // never enforced by either engine, so no padding or alignment logic is
    // needed here.

    /// Load a single-limb value of `ty` from `addr`, exactly `ty.size_in_bytes()`
    /// bytes (little-endian), and store the result cell.
    fn lower_load_single(
        &self,
        ty: IrType,
        addr: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        // Push the loaded value cell(s) onto the operand stack.
        match ty {
            IrType::I32 | IrType::Pointer => {
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::Load);
            }
            IrType::I1 => {
                // Load the byte; a nonzero byte is the boolean true (the
                // interpreter reads `bytes[0] != 0`). `Load8` yields 0..255, so
                // `byte > 0` ⟺ `byte != 0`, producing a genuine i1 flag that
                // `Branch`/`select` can consume.
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::Load8);
                f.push(IsaInstr::ConstI32(0));
                f.push(IsaInstr::Gt);
            }
            IrType::I8 => {
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::Load8);
            }
            IrType::I16 => {
                // lo = Load8(addr), hi = Load8(addr + 1); value = (hi << 8) | lo.
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::Load8); // lo
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::ConstI32(1));
                f.push(IsaInstr::I32Add);
                f.push(IsaInstr::Load8); // hi
                f.push(IsaInstr::ConstI32(8));
                f.push(IsaInstr::Shl);
                f.push(IsaInstr::Or);
            }
            _ => {
                return Err(LowerError::UnsupportedType(format!(
                    "single-limb load of {ty} is not supported on the VM backend"
                )))
            }
        }
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    /// Store a single-limb `value` of type `ty` to `addr`, writing exactly
    /// `ty.size_in_bytes()` bytes (little-endian).
    ///
    /// The stored cell is the interpreter's wider-carrier convention: a sub-word
    /// value may travel in an `I32` word whose low `w` bits are the value, so
    /// each byte written is masked to its 8-bit slice of the value's width — the
    /// byte count and the byte values both derive from `ty`, never from the
    /// carrier.
    fn lower_store_single(
        &self,
        ty: IrType,
        value: ValueId,
        addr: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        match ty {
            // Word-width stores keep the VM's word `Store` (the value cell is an
            // `I32`/pointer of exactly 4 bytes).
            IrType::I32 | IrType::Pointer => {
                // The VM expects the value on top of the operand stack and the
                // address underneath it, so emit the address first.
                self.emit_load(addr, ctx, f)?;
                self.emit_load(value, ctx, f)?;
                f.push(IsaInstr::Store);
            }
            // Sub-word stores write `Store8`s: `Store8` masks its byte to `&0xFF`
            // (and accepts an `I1` flag cell), so the low byte is exact even when
            // the carrier holds garbage above the value's width.
            IrType::I1 | IrType::I8 => {
                self.emit_load(addr, ctx, f)?;
                self.emit_load(value, ctx, f)?;
                f.push(IsaInstr::Store8);
            }
            IrType::I16 => {
                // lo = value & 0xFF at addr, hi = (value >> 8) & 0xFF at
                // addr + 1. The carrier is masked to 16 bits before splitting so
                // both bytes are exact regardless of any higher garbage bits.
                self.emit_load(addr, ctx, f)?;
                self.emit_load(value, ctx, f)?;
                f.push(IsaInstr::ConstI32(0xFF));
                f.push(IsaInstr::And);
                f.push(IsaInstr::Store8); // low byte at addr
                self.emit_load(addr, ctx, f)?;
                f.push(IsaInstr::ConstI32(1));
                f.push(IsaInstr::I32Add); // addr + 1
                self.emit_load(value, ctx, f)?;
                f.push(IsaInstr::ConstI32(0xFFFF));
                f.push(IsaInstr::And);
                f.push(IsaInstr::ConstI32(8));
                f.push(IsaInstr::Shr);
                f.push(IsaInstr::Store8); // high byte at addr + 1
            }
            _ => {
                return Err(LowerError::UnsupportedType(format!(
                    "single-limb store of {ty} is not supported on the VM backend"
                )))
            }
        }
        Ok(())
    }

    /// Lower a width-changing integer cast (LLVM `zext`/`sext`/`trunc`) to
    /// limb arithmetic. Bitcasts and pointer-integer reinterpretations that are
    /// representation-preserving under the target profile are lowered by
    /// [`IsaLowerer::lower_reinterpret_cast`] as zero-cost cell operations;
    /// those that would require a value transformation LLVM does not call for
    /// are rejected explicitly (nothing is approximated).
    ///
    /// The `result_id: Option<ValueId>` makes the arity (op + value + optional
    /// result + ctx + emitter + label counter) exceed clippy's default,
    /// matching the other `lower_*` helpers' threading style.
    #[allow(clippy::too_many_arguments)]
    fn lower_cast(
        &self,
        op: CastOp,
        from_ty: IrType,
        to_ty: IrType,
        value: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
        counter: &mut usize,
    ) -> Result<(), LowerError> {
        match op {
            CastOp::Zext | CastOp::Sext | CastOp::Trunc => {
                self.lower_int_width_cast(op, from_ty, to_ty, value, result_id, ctx, f)
            }
            CastOp::Bitcast | CastOp::PtrToInt | CastOp::IntToPtr => self
                .lower_reinterpret_cast(op, from_ty, to_ty, value, result_id, ctx, f, counter),
        }
    }

    /// Lower a width-changing integer cast (`zext`/`sext`/`trunc`). These operate
    /// on integer types only; pointer-typed forms are the reinterpret casts.
    #[allow(clippy::too_many_arguments)]
    fn lower_int_width_cast(
        &self,
        op: CastOp,
        from_ty: IrType,
        to_ty: IrType,
        value: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(fw) = from_ty.integer_width() else {
            return Err(LowerError::UnsupportedInstruction(format!(
                "{} from {} is not supported on the VM backend (integer casts only)",
                op.name(),
                from_ty,
            )));
        };
        let Some(tw) = to_ty.integer_width() else {
            return Err(LowerError::UnsupportedInstruction(format!(
                "{} to {} is not supported on the VM backend (integer casts only)",
                op.name(),
                to_ty,
            )));
        };
        match op {
            CastOp::Zext | CastOp::Sext => {
                if tw <= fw {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "{} requires widening, got {} -> {}",
                        op.name(),
                        from_ty,
                        to_ty,
                    )));
                }
            }
            CastOp::Trunc => {
                if tw >= fw {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "trunc requires narrowing, got {} -> {}",
                        from_ty,
                        to_ty,
                    )));
                }
            }
            _ => unreachable!(),
        }

        let Some(id) = result_id else { return Ok(()) };
        let q = ctx.first_slot(id)?;

        // ---- Target is 64 bits: widening (zext/sext) into two result cells.
        // The source is single-cell (fw < 64 for a widening cast).
        if tw == 64 {
            match op {
                CastOp::Sext => {
                    // Low cell = the source widened into 32 bits; high cell =
                    // the sign fill. A 32-bit source already fills its limb and
                    // passes through unchanged — OR-ing it with the all-ones
                    // sign fill would corrupt negatives (sext i32 -7: 0xFFFFFFF9
                    // | 0xFFFFFFFF = 0xFFFFFFFF = -1). Only fw < 32 sources need
                    // the fill OR'd in to sign-extend them into the low limb.
                    if fw == 32 {
                        self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
                        f.push(IsaInstr::LocalSet(q));
                    } else {
                        self.emit_sext_low32(fw, from_ty, value, ctx, f)?;
                        f.push(IsaInstr::LocalSet(q));
                    }
                    self.emit_sext_fill(fw, from_ty, value, ctx, f)?;
                    f.push(IsaInstr::LocalSet(q + 1));
                }
                _ => {
                    // zext: low cell = source, high cell = 0.
                    self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
                    f.push(IsaInstr::LocalSet(q));
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::LocalSet(q + 1));
                }
            }
            return Ok(());
        }

        // ---- Single-cell target (tw in 1|8|16|32).
        match op {
            CastOp::Trunc => {
                // The low bits of the source carry the truncated value.
                if fw == 64 {
                    self.emit_load_limb(value, 0, ctx, f)?;
                } else {
                    self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
                }
                match tw {
                    32 => { /* the cell is already exactly 32 bits */ }
                    1 => {
                        f.push(IsaInstr::ConstI32(1));
                        f.push(IsaInstr::And);
                        self.reduce_top_to_flag(f);
                    }
                    _ => self.mask_top_to(tw, f),
                }
            }
            CastOp::Sext => {
                // Sign-extend a single-cell source into the target cell:
                // low = source | (0 - (source >> (fw-1))), masked to `tw` bits
                // (the interpreter carries sub-32 results masked to their width).
                self.emit_sext_low32(fw, from_ty, value, ctx, f)?;
                if tw < 32 {
                    self.mask_top_to(tw, f);
                }
            }
            _ => {
                // zext into a <=32-bit cell. Sub-32 sources travel masked to fw
                // bits, so the cell already equals the extended value.
                self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
            }
        }
        if let Some(id) = result_id {
            self.emit_store(id, ctx, f)?;
        }
        Ok(())
    }

    /// Lower a representation-preserving reinterpretation (`bitcast`,
    /// `ptrtoint`, `inttoptr`) to zero-cost cell moves on the VM.
    ///
    /// SA48 pointers are single 32-bit cells, so every reinterpretation the
    /// LLVM frontend can express is a pure cell copy — never a value
    /// transformation:
    ///
    /// - `ptrtoint ptr → i32` copies the pointer cell (canonicalized to an `I32`
    ///   word); `ptrtoint ptr → i64` zero-extends it into the `(low, high)` limb
    ///   pair.
    /// - `inttoptr` from a single-cell integer copies the cell into the pointer
    ///   slot; from an `i64` it keeps the low limb and **traps** when the high
    ///   limb is nonzero, mirroring the interpreter's refusal to silently
    ///   truncate an address that does not fit the 32-bit pointer.
    /// - `bitcast` is accepted only when it is a genuine no-op reinterpretation
    ///   (equal-size, equal-kind: pointer→pointer or same-width integer→integer)
    ///   and copies the source cells through.
    ///
    /// Forms that would require a value transformation are rejected explicitly
    /// (never approximated), with a diagnostic naming the offending conversion.
    #[allow(clippy::too_many_arguments)]
    fn lower_reinterpret_cast(
        &self,
        op: CastOp,
        from_ty: IrType,
        to_ty: IrType,
        value: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
        counter: &mut usize,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        let q = ctx.first_slot(id)?;

        // ---- Form validation (mirrors the interpreter's `cast_value`). ----
        match op {
            CastOp::PtrToInt => {
                if from_ty != IrType::Pointer {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "ptrtoint source must be a pointer, got {from_ty}"
                    )));
                }
                if !matches!(to_ty, IrType::I32 | IrType::I64) {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "ptrtoint target must be a pointer-sized integer (i32/i64), got {to_ty}"
                    )));
                }
            }
            CastOp::IntToPtr => {
                if to_ty != IrType::Pointer {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "inttoptr target must be a pointer, got {to_ty}"
                    )));
                }
                if !from_ty.is_integer() && from_ty != IrType::Pointer {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "inttoptr source must be an integer or pointer, got {from_ty}"
                    )));
                }
            }
            CastOp::Bitcast => {
                if from_ty.size_in_bytes() != to_ty.size_in_bytes() {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "bitcast requires equal byte sizes, got {from_ty} -> {to_ty}"
                    )));
                }
                // Only a same-kind, same-width reinterpretation preserves the
                // cell bit pattern. Pointer↔integer bitcasts are not valid LLVM
                // IR at all, and an `f64` cell has no faithful word form here.
                let same_width_int = from_ty.is_integer()
                    && to_ty.is_integer()
                    && from_ty.integer_width() == to_ty.integer_width();
                let ptr_to_ptr = from_ty == IrType::Pointer && to_ty == IrType::Pointer;
                if !same_width_int && !ptr_to_ptr {
                    return Err(LowerError::UnsupportedInstruction(format!(
                        "bitcast ({from_ty} -> {to_ty}) is not a representation-preserving \
                         reinterpretation on the VM backend"
                    )));
                }
            }
            _ => unreachable!(),
        }

        // ---- Emission. ----
        match op {
            CastOp::PtrToInt => match to_ty {
                // Address -> single i32 cell (canonical word so the result can
                // reach the strict `i32` word ops).
                IrType::I32 => {
                    self.emit_load(value, ctx, f)?;
                    self.canonicalize_top_word(f);
                    f.push(IsaInstr::LocalSet(q));
                }
                // Address -> i64: zero-extend into the (low, high) limb pair.
                IrType::I64 => {
                    self.emit_load(value, ctx, f)?;
                    self.canonicalize_top_word(f);
                    f.push(IsaInstr::LocalSet(q));
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::LocalSet(q + 1));
                }
                _ => unreachable!(),
            },
            CastOp::IntToPtr => {
                if from_ty == IrType::I64 {
                    // The address must fit the 32-bit pointer. Trap when the
                    // high limb is nonzero instead of silently truncating it
                    // (the interpreter rejects the same value); otherwise the
                    // low limb is the address.
                    let n = *counter;
                    *counter += 1;
                    let ok_label = format!("__inttoptr_{n}_ok");
                    let fail_label = format!("__inttoptr_{n}_fail");
                    self.emit_load_limb(value, 1, ctx, f)?;
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::Eq); // i1 flag: high limb == 0
                    f.push(IsaInstr::Branch(ok_label.clone(), fail_label.clone()));
                    f.set_label(&fail_label);
                    f.push(IsaInstr::Trap);
                    f.set_label(&ok_label);
                    self.emit_load_limb(value, 0, ctx, f)?;
                    self.canonicalize_top_word(f);
                    f.push(IsaInstr::LocalSet(q));
                } else {
                    // Single-cell source (i1/i8/i16/i32, or a pointer
                    // passthrough in degenerate IR): copy into the pointer slot.
                    self.emit_load(value, ctx, f)?;
                    self.canonicalize_top_word(f);
                    f.push(IsaInstr::LocalSet(q));
                }
            }
            CastOp::Bitcast => {
                // Same-kind, same-width no-op: copy the source cells through to
                // the result slot (one cell, or two for an i64↔i64 form).
                self.emit_load(value, ctx, f)?;
                self.emit_store(id, ctx, f)?;
            }
            _ => unreachable!(), // routed to lower_int_width_cast
        }
        Ok(())
    }

    /// Push the single-cell integer `value` as an I32 word, widening an i1
    /// (whose slot holds an `I1` value) by adding it to a zero constant.
    fn emit_cast_source_as_i32(
        &self,
        from_ty: IrType,
        value: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        self.emit_load(value, ctx, f)?;
        if from_ty == IrType::I1 {
            self.canonicalize_top_word(f);
        }
        Ok(())
    }

    /// Leave `0 - (source >> (fw-1))` on the stack: the all-ones/all-zeros sign
    /// fill of a single-cell integer source of width `fw`.
    fn emit_sext_fill(
        &self,
        fw: u32,
        from_ty: IrType,
        value: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        f.push(IsaInstr::ConstI32(0));
        self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
        f.push(IsaInstr::ConstI32(fw - 1));
        f.push(IsaInstr::Shr);
        f.push(IsaInstr::I32Sub);
        Ok(())
    }

    /// Leave `source | sign_fill` on the stack: the sign extension of the
    /// single-cell source into a full 32-bit cell.
    fn emit_sext_low32(
        &self,
        fw: u32,
        from_ty: IrType,
        value: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        self.emit_sext_fill(fw, from_ty, value, ctx, f)?;
        self.emit_cast_source_as_i32(from_ty, value, ctx, f)?;
        f.push(IsaInstr::Or);
        Ok(())
    }

    /// Mask the 32-bit word on top of the stack to `width` bits (`1|8|16`).
    fn mask_top_to(&self, width: u32, f: &mut FuncEmitter<'_>) {
        let mask = ((1u64 << width) - 1) as u32;
        f.push(IsaInstr::ConstI32(mask));
        f.push(IsaInstr::And);
    }

    // ---- Bitwise and shift lowering ---------------------------------------
    //
    // The ISA has 32-bit word `And`/`Or`/`Xor` and logical `Shl`/`Shr`. SAIR
    // bitwise ops are width-preserving, so a single-cell (i1/i8/i16/i32) value
    // maps to the word op directly and a two-limb i64 maps to per-limb word ops.
    //
    // Shifts need width-aware semantics the word ops cannot express directly:
    // a two-limb (64-bit) value shifts across the limb boundary, and an
    // arithmetic shift must replicate a width-derived sign. Both are realised
    // by calling a small software shift helper appended to the program (see
    // [`build_shift_helper`]): the value is widened to a (lo, hi) pair and the
    // helper steps one bit at a time in a loop. The effective shift amount is
    // `amount & (width - 1)` (amount mod width), the deterministic poison-region
    // choice that matches the interpreter (`EXECUTION_MODEL.md` §5.7).

    /// Mask the single-cell result on top of the stack to the integer width of
    /// `ty` and store it into the result slot. Sub-32-bit results are kept
    /// canonical (masked to their width) so later full-cell comparisons match
    /// the interpreter's masked reads; an i1 result is reduced to a true flag.
    fn store_masked_single(
        &self,
        ty: IrType,
        id: ValueId,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        match ty.integer_width() {
            Some(1) => {
                f.push(IsaInstr::ConstI32(1));
                f.push(IsaInstr::And);
                self.reduce_top_to_flag(f);
            }
            Some(w) if w < 32 => self.mask_top_to(w, f),
            _ => {}
        }
        self.emit_store(id, ctx, f)?;
        Ok(())
    }

    /// Lower a single-cell shift (`i1|i8|i16|i32`) to a call on the matching
    /// 64-bit software helper. The operand is sign- or zero-extended into the
    /// helper's `(lo, hi)` pair and the helper's low-limb result is truncated
    /// back to the cell's width.
    #[allow(clippy::too_many_arguments)] // result_id + ctx + emitter idiom (see emit_gep_addr)
    fn lower_shift_single(
        &self,
        ty: IrType,
        kind: ShiftKind,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let w = ty.integer_width().unwrap_or(32);
        // Canonical operand: widen i1 to a word and mask sub-32 carriers so the
        // sign computation reads clean low bits, then park it in the temp slot
        // (the helper argument order needs the low limb pushed before the fill).
        self.emit_cast_source_as_i32(ty, lhs, ctx, f)?;
        if w < 32 {
            self.mask_top_to(w, f);
        }
        f.push(IsaInstr::LocalSet(ctx.temp_slot));

        // Low limb and high fill. For shl/lshr the value is zero-extended
        // (hi = 0). For ashr the width-bit sign must be replicated across the
        // *whole* (lo, hi) pair — bits w..31 of the low limb as well as the high
        // limb — so the helper shifts the true sign-extended 64-bit pattern, not
        // a value whose upper low-limb bits are zero.
        match kind {
            ShiftKind::Shl | ShiftKind::Lshr => {
                f.push(IsaInstr::LocalGet(ctx.temp_slot));
                f.push(IsaInstr::ConstI32(0));
            }
            ShiftKind::Ashr => {
                if w < 32 {
                    // S = 0 - (M >> (w-1)): all-ones when the width bit is set.
                    // low_ext = M | (S & fill), where fill has bits w..31 set.
                    let upper_fill = (u32::MAX >> w) << w;
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::LocalGet(ctx.temp_slot));
                    f.push(IsaInstr::ConstI32(w - 1));
                    f.push(IsaInstr::Shr);
                    f.push(IsaInstr::I32Sub);
                    f.push(IsaInstr::ConstI32(upper_fill));
                    f.push(IsaInstr::And);
                    f.push(IsaInstr::LocalGet(ctx.temp_slot));
                    f.push(IsaInstr::Or);
                    // hi = S, recomputed (M is still parked in the temp slot).
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::LocalGet(ctx.temp_slot));
                    f.push(IsaInstr::ConstI32(w - 1));
                    f.push(IsaInstr::Shr);
                    f.push(IsaInstr::I32Sub);
                } else {
                    // w == 32: the low limb already carries the full pattern.
                    f.push(IsaInstr::LocalGet(ctx.temp_slot));
                    // 0 - (M >> 31): all-ones when bit 31 is set.
                    f.push(IsaInstr::ConstI32(0));
                    f.push(IsaInstr::LocalGet(ctx.temp_slot));
                    f.push(IsaInstr::ConstI32(w - 1));
                    f.push(IsaInstr::Shr);
                    f.push(IsaInstr::I32Sub);
                }
            }
        }
        // Amount masked to width (amount mod width). An i1 amount is widened to
        // a word first so the ISA `And` sees an integer cell.
        self.emit_cast_source_as_i32(ty, rhs, ctx, f)?;
        f.push(IsaInstr::ConstI32(w - 1));
        f.push(IsaInstr::And);
        // Call helper, drop the (irrelevant) high limb, keep the low limb.
        f.push(IsaInstr::Call(kind.helper().to_string()));
        f.push(IsaInstr::Drop);
        if let Some(id) = result_id {
            self.store_masked_single(ty, id, ctx, f)?;
        }
        Ok(())
    }

    /// Lower a two-limb (i64) shift to a call on the matching software helper.
    /// The amount's low limb is masked to 63 bits (`amount mod 64`), matching
    /// the interpreter.
    fn lower_shift_wide(
        &self,
        kind: ShiftKind,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        let q = ctx.first_slot(id)?;
        let lhs_first = ctx.first_slot(lhs)?;
        let rhs_first = ctx.first_slot(rhs)?;
        // (lo, hi, amount & 63)
        f.push(IsaInstr::LocalGet(lhs_first));
        f.push(IsaInstr::LocalGet(lhs_first + 1));
        f.push(IsaInstr::LocalGet(rhs_first));
        f.push(IsaInstr::ConstI32(63));
        f.push(IsaInstr::And);
        f.push(IsaInstr::Call(kind.helper().to_string()));
        // Stack: lo', hi' (hi' on top) — store hi' then lo'.
        f.push(IsaInstr::LocalSet(q + 1));
        f.push(IsaInstr::LocalSet(q));
        Ok(())
    }

    /// Lower a two-limb (i64) `and`/`or`/`xor` limb-wise.
    #[allow(clippy::too_many_arguments)] // isa_op + result_id + ctx + emitter idiom
    fn lower_wide_bitwise(
        &self,
        _ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        isa_op: IsaInstr,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        let q = ctx.first_slot(id)?;
        let lhs_first = ctx.first_slot(lhs)?;
        let rhs_first = ctx.first_slot(rhs)?;
        for limb in 0..2 {
            f.push(IsaInstr::LocalGet(lhs_first + limb));
            f.push(IsaInstr::LocalGet(rhs_first + limb));
            f.push(isa_op.clone());
            f.push(IsaInstr::LocalSet(q + limb));
        }
        Ok(())
    }

    /// Lower an SAIR `Mul` whose type occupies two limbs (i64) to a call on the
    /// [`__sair_mul64`] software helper.
    ///
    /// The helper realises `(a0 + a1·2^32) · (b0 + b1·2^32) mod 2^64` from 32-bit
    /// word `I32Mul` plus 16-bit schoolbook carries (there is no widening ISA
    /// multiply). It takes the four operand limbs on the stack (`a0 a1 b0 b1`,
    /// `b1` on top) and returns the product pair `(lo, hi)` with the high limb on
    /// top.
    fn lower_wide_mul(
        &self,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        self.emit_load(lhs, ctx, f)?; // a0 a1
        self.emit_load(rhs, ctx, f)?; // b0 b1 (on top)
        f.push(IsaInstr::Call(MUL64_HELPER.to_string()));
        let q = ctx.first_slot(id)?;
        // Stack: lo hi (hi on top) — store hi then lo.
        f.push(IsaInstr::LocalSet(q + 1));
        f.push(IsaInstr::LocalSet(q));
        Ok(())
    }

    /// Lower an SAIR `Div`/`Rem` whose type occupies two limbs (i64) to a call on
    /// the [`__sair_udivrem64`] software helper.
    ///
    /// The helper computes both the unsigned quotient and the unsigned remainder
    /// of two 64-bit magnitudes by a 64-step bit-at-a-time restoring division
    /// (see `build_udivrem_helper`), and returns `(r0 r1 q0 q1)` with the
    /// quotient pair on top. `Div` keeps the quotient and drops the remainder;
    /// `Rem` drops the quotient and keeps the remainder. SAIR `Div`/`Rem` are
    /// unsigned (LLVM signed forms are already expanded to magnitudes by the
    /// frontend), so one helper serves both.
    #[allow(clippy::too_many_arguments)] // kind + result_id + ctx + emitter idiom
    fn lower_wide_divrem(
        &self,
        kind: DivRemKind,
        lhs: ValueId,
        rhs: ValueId,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        let Some(id) = result_id else { return Ok(()) };
        self.emit_load(lhs, ctx, f)?; // a0 a1
        self.emit_load(rhs, ctx, f)?; // b0 b1 (on top)
        f.push(IsaInstr::Call(UDIVREM_HELPER.to_string()));
        // Stack after the call: r0 r1 q0 q1 (quotient pair on top).
        let q = ctx.first_slot(id)?;
        match kind {
            DivRemKind::Div => {
                // Keep q0 q1 (store hi then lo), discard the remainder pair.
                f.push(IsaInstr::LocalSet(q + 1));
                f.push(IsaInstr::LocalSet(q));
                f.push(IsaInstr::Drop);
                f.push(IsaInstr::Drop);
            }
            DivRemKind::Rem => {
                // Discard q1 q0, keep the remainder pair (store hi then lo).
                f.push(IsaInstr::Drop);
                f.push(IsaInstr::Drop);
                f.push(IsaInstr::LocalSet(q + 1));
                f.push(IsaInstr::LocalSet(q));
            }
        }
        Ok(())
    }
}

/// Which shift operation a [`IsaLowerer`] shift arm is lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShiftKind {
    Shl,
    Lshr,
    Ashr,
}

impl ShiftKind {
    /// The name of the program-level software helper realising this shift.
    fn helper(&self) -> &'static str {
        match self {
            ShiftKind::Shl => "__sair_shl64",
            ShiftKind::Lshr => "__sair_lshr64",
            ShiftKind::Ashr => "__sair_ashr64",
        }
    }
}

/// The shift helpers, in canonical program-append order.
const SHIFT_HELPERS: [&str; 3] = ["__sair_shl64", "__sair_lshr64", "__sair_ashr64"];

/// Whether `module` contains an SAIR shift that lowers to `helper`.
fn module_uses_shift(module: &IrModule, helper: &str) -> bool {
    module.functions.iter().any(|func| {
        func.blocks.iter().any(|block| {
            block.instructions.iter().any(|instr| {
                matches!(
                    (helper, instr),
                    ("__sair_shl64", SairInstr::Shl { .. })
                        | ("__sair_lshr64", SairInstr::Lshr { .. })
                        | ("__sair_ashr64", SairInstr::Ashr { .. })
                )
            })
        })
    })
}

// ---- Two-limb multiply/divide software helpers ------------------------------
//
// The ISA multiplies and divides 32-bit words only. A 64-bit (two-limb) SAIR
// `Mul`/`Div`/`Rem` therefore lowers to a call on a program-level software
// helper built entirely from the existing word primitives (see the two builder
// functions below). Only the helpers a module actually references are appended.

/// Program-level helper realising a two-limb wrapping multiply (`mod 2^64`).
const MUL64_HELPER: &str = "__sair_mul64";
/// Program-level helper realising two-limb unsigned divide/remainder together.
const UDIVREM_HELPER: &str = "__sair_udivrem64";

/// Which result an SAIR `Div`/`Rem` instruction keeps from [`UDIVREM_HELPER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DivRemKind {
    Div,
    Rem,
}

/// Whether `module` lowers an SAIR `Mul` at 64-bit width (i.e. calls
/// [`MUL64_HELPER`]). Sub-64 multiplies are single-cell word `I32Mul`s and are
/// deliberately not counted.
fn module_uses_wide_mul(module: &IrModule) -> bool {
    module.functions.iter().any(|func| {
        func.blocks.iter().any(|block| {
            block.instructions.iter().any(|instr| {
                matches!(instr, SairInstr::Mul { ty: IrType::I64, .. })
            })
        })
    })
}

/// Whether `module` lowers an SAIR `Div`/`Rem` at 64-bit width (i.e. calls
/// [`UDIVREM_HELPER`]). Sub-64 divisions are single-cell `I32Div`/`I32Rem`s and
/// are deliberately not counted.
fn module_uses_wide_divrem(module: &IrModule) -> bool {
    module.functions.iter().any(|func| {
        func.blocks.iter().any(|block| {
            block.instructions.iter().any(|instr| {
                matches!(
                    instr,
                    SairInstr::Div { ty: IrType::I64, .. } | SairInstr::Rem { ty: IrType::I64, .. }
                )
            })
        })
    })
}

/// Build the program-level two-limb multiply helper.
///
/// Signature: takes `(a_lo, a_hi, b_lo, b_hi)` on the operand stack (`b_hi` on
/// top, matching the `param_cells = 4` prologue convention) and returns the
/// 64-bit product `mod 2^64` as `(lo, hi)` with the high limb on top
/// (`return_cells = 2`).
///
/// The ISA only multiplies 32-bit words (`I32Mul`, which yields the low 32 bits
/// of the 64-bit product), so the product
/// `(a0 + a1·2^32)·(b0 + b1·2^32) mod 2^64` is expanded over 16-bit schoolbook
/// digits, where every partial product fits in a word exactly:
///
/// ```text
/// P = a0·b0 + (a0·b1 + a1·b0)·2^32  (mod 2^64; the a1·b1·2^64 term vanishes)
/// result_lo = low32(a0·b0)
/// result_hi = high32(a0·b0) + low32(a0·b1) + low32(a1·b0)  (mod 2^32)
/// ```
///
/// `high32(a0·b0)` — the only full 64-bit product needed — is itself formed from
/// four exact 16×16→32 products `c0..c3` with carry propagation across 16-bit
/// digit positions (see the code).
fn build_mul_helper() -> Function {
    use IsaInstr as I;

    const A0: u32 = 0;
    const A1: u32 = 1;
    const B0: u32 = 2;
    const B1: u32 = 3;
    const XL: u32 = 4;
    const XH: u32 = 5;
    const YL: u32 = 6;
    const YH: u32 = 7;
    const C0: u32 = 8;
    const C1: u32 = 9;
    const C2: u32 = 10;
    const C3: u32 = 11;
    const SUM1: u32 = 12;
    const RLO: u32 = 13;
    const RHI: u32 = 14;
    const SUM2: u32 = 15;

    let mut f = Function::new(MUL64_HELPER);
    f.param_cells = 4;
    f.local_count = 16;
    f.return_cells = 2;

    // Prologue: pop (a_lo, a_hi, b_lo, b_hi) into slots 0..3.
    for slot in (0..4).rev() {
        f.push(None::<&str>, I::LocalSet(slot));
    }

    // Split each low operand limb into 16-bit halves.
    f.push(None::<&str>, I::LocalGet(A0));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::LocalSet(XL));
    f.push(None::<&str>, I::LocalGet(A0));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::LocalSet(XH));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::LocalSet(YL));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::LocalSet(YH));

    // Exact 16×16→32 partial products of a0·b0.
    f.push(None::<&str>, I::LocalGet(XL));
    f.push(None::<&str>, I::LocalGet(YL));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalSet(C0));
    f.push(None::<&str>, I::LocalGet(XL));
    f.push(None::<&str>, I::LocalGet(YH));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalSet(C1));
    f.push(None::<&str>, I::LocalGet(XH));
    f.push(None::<&str>, I::LocalGet(YL));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalSet(C2));
    f.push(None::<&str>, I::LocalGet(XH));
    f.push(None::<&str>, I::LocalGet(YH));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalSet(C3));

    // sum1 = (c0 >> 16) + (c1 & 0xFFFF) + (c2 & 0xFFFF)  (< 2^18, exact).
    f.push(None::<&str>, I::LocalGet(C0));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::LocalGet(C1));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(C2));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(SUM1));

    // result_lo = (c0 & 0xFFFF) | ((sum1 & 0xFFFF) << 16): low 32 bits of a0·b0.
    f.push(None::<&str>, I::LocalGet(C0));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::LocalGet(SUM1));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shl);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(RLO));

    // sum2 = (sum1 >> 16) + (c1 >> 16) + (c2 >> 16) + (c3 & 0xFFFF) (< 2^18).
    f.push(None::<&str>, I::LocalGet(SUM1));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::LocalGet(C1));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(C2));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(C3));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(SUM2));

    // high32(a0·b0) = (sum2 & 0xFFFF) | (((sum2 >> 16) + (c3 >> 16)) << 16).
    f.push(None::<&str>, I::LocalGet(SUM2));
    f.push(None::<&str>, I::ConstI32(0xFFFF));
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::LocalGet(SUM2));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::LocalGet(C3));
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::ConstI32(16));
    f.push(None::<&str>, I::Shl);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(RHI));

    // result_hi = high32(a0·b0) + low32(a0·b1) + low32(a1·b0) (mod 2^32).
    f.push(None::<&str>, I::LocalGet(A0));
    f.push(None::<&str>, I::LocalGet(B1));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalGet(RHI));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(RHI));
    f.push(None::<&str>, I::LocalGet(A1));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::LocalGet(RHI));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(RHI));

    // Return (lo, hi) — hi on top.
    f.push(None::<&str>, I::LocalGet(RLO));
    f.push(None::<&str>, I::LocalGet(RHI));
    f.push(None::<&str>, I::Return);
    f
}

/// Build the program-level two-limb unsigned divide/remainder helper.
///
/// Signature: takes `(a_lo, a_hi, b_lo, b_hi)` on the operand stack (`b_hi` on
/// top, `param_cells = 4`) and returns the quotient and remainder of the
/// unsigned 64-bit division `a / b` as `(r_lo, r_hi, q_lo, q_hi)` with the
/// quotient pair on top (`return_cells = 4`). SAIR `Div`/`Rem` are unsigned, so
/// one helper serves both a `udiv`/`urem` mapping and the magnitude step of the
/// frontend's signed expansion.
///
/// A divisor of zero is detected up front and reported exactly like the ISA's
/// word `I32Div`: it executes a manufactured `0 / 0` word division, which the
/// VM deterministically fails with `VmError::DivisionByZero` (the same variant
/// an i32 `udiv x, 0` raises). The interpreter surfaces the equal
/// `InterpError::DivisionByZero` on the same module.
///
/// Quotient and remainder come from a 64-step restoring division: each iteration
/// brings the dividend's next bit (drained from the top of `a` by shifting it
/// left) into a running remainder `r = (r << 1) | bit`; when `r >= b` the
/// divisor is subtracted and the quotient bit is set. The quotient is built
/// most-significant-bit-first by appending each new bit at the bottom of `q`
/// while shifting `q` left, so after 64 iterations `q` holds the exact quotient
/// and `r` the exact remainder. Every step is branchless except the loop
/// itself — comparisons select 0/1 and the subtract is masked by the "r >= b"
/// flag (`sub = ge ? b : 0`).
///
/// Loop layout (labels are function-local):
/// ```text
/// prologue: pop b_hi b_lo a_hi a_lo → slots; count = 64
/// zero:     (b_lo | b_hi) == 0 ? div0 : chk
/// div0:     0 / 0               // manufactured DivisionByZero
/// chk:      count == 0 ? done : body
/// body:     one restoring-division step (below); goto chk
/// done:     return r_lo r_hi q_lo q_hi (q on top)
/// ```
fn build_udivrem_helper() -> Function {
    use IsaInstr as I;

    const A0: u32 = 0;
    const A1: u32 = 1;
    const B0: u32 = 2;
    const B1: u32 = 3;
    const R0: u32 = 4;
    const R1: u32 = 5;
    const Q0: u32 = 6;
    const Q1: u32 = 7;
    const CNT: u32 = 8;
    const GE: u32 = 9;
    const BRW: u32 = 10;

    let mut f = Function::new(UDIVREM_HELPER);
    f.param_cells = 4;
    f.local_count = 11;
    f.return_cells = 4;

    // Prologue: pop (a_lo, a_hi, b_lo, b_hi) into slots 0..3.
    for slot in (0..4).rev() {
        f.push(None::<&str>, I::LocalSet(slot));
    }

    // Exactly 64 restoring-division iterations.
    f.push(None::<&str>, I::ConstI32(64));
    f.push(None::<&str>, I::LocalSet(CNT));

    // Division-by-zero guard: (b_lo | b_hi) == 0 → div0.
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::LocalGet(B1));
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::Eq);
    f.push(None::<&str>, I::Branch("div0".to_string(), "chk".to_string()));

    // Manufactured DivisionByZero: a 0/0 word division always errors in the VM.
    f.push(Some("div0"), I::ConstI32(0));
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::I32Div);

    // Loop test.
    f.push(Some("chk"), I::LocalGet(CNT));
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::Eq);
    f.push(None::<&str>, I::Branch("done".to_string(), "body".to_string()));

    // ---- One restoring-division step ----
    // r1' = (r1 << 1) | (r0 >> 31)
    f.push(Some("body"), I::LocalGet(R1));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(R0));
    f.push(None::<&str>, I::ConstI32(31));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(R1));
    // r0' = (r0 << 1) | (a1 >> 31): bring down the dividend's next (top) bit.
    f.push(None::<&str>, I::LocalGet(R0));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(A1));
    f.push(None::<&str>, I::ConstI32(31));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(R0));
    // a1' = (a1 << 1) | (a0 >> 31): drain a's next bit toward the top.
    f.push(None::<&str>, I::LocalGet(A1));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(A0));
    f.push(None::<&str>, I::ConstI32(31));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(A1));
    // a0' = a0 << 1
    f.push(None::<&str>, I::LocalGet(A0));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(A0));

    // ge = widen( !(r <u b) ), where
    //   lt = (r1 < b1) | ((r1 == b1) & (r0 < b0)).
    f.push(None::<&str>, I::LocalGet(R1));
    f.push(None::<&str>, I::LocalGet(B1));
    f.push(None::<&str>, I::Lt);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(R1));
    f.push(None::<&str>, I::LocalGet(B1));
    f.push(None::<&str>, I::Eq);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(R0));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::Lt);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::And);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::Eq);
    f.push(None::<&str>, I::ConstI32(0));
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalSet(GE));

    // subtract the divisor only when r >= b: sub = ge ? b : 0, formed with a
    // word multiply (ge is exactly 0 or 1, so ge * b selects b or 0; a bitwise
    // And would wrongly mask off bits of a multi-bit limb).
    // borrow = r0 <u sub0   (an i1 flag)
    f.push(None::<&str>, I::LocalGet(R0));
    f.push(None::<&str>, I::LocalGet(GE));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::Lt);
    f.push(None::<&str>, I::LocalSet(BRW));
    // r0' = r0 - (ge * b0)
    f.push(None::<&str>, I::LocalGet(R0));
    f.push(None::<&str>, I::LocalGet(GE));
    f.push(None::<&str>, I::LocalGet(B0));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::I32Sub);
    f.push(None::<&str>, I::LocalSet(R0));
    // r1' = r1 - (ge * b1) - borrow
    f.push(None::<&str>, I::LocalGet(R1));
    f.push(None::<&str>, I::LocalGet(GE));
    f.push(None::<&str>, I::LocalGet(B1));
    f.push(None::<&str>, I::I32Mul);
    f.push(None::<&str>, I::I32Sub);
    f.push(None::<&str>, I::LocalGet(BRW));
    f.push(None::<&str>, I::I32Sub);
    f.push(None::<&str>, I::LocalSet(R1));

    // q1' = (q1 << 1) | (q0 >> 31); the quotient bit is appended at the bottom.
    f.push(None::<&str>, I::LocalGet(Q1));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(Q0));
    f.push(None::<&str>, I::ConstI32(31));
    f.push(None::<&str>, I::Shr);
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(Q1));
    // q0' = (q0 << 1) | ge
    f.push(None::<&str>, I::LocalGet(Q0));
    f.push(None::<&str>, I::Dup);
    f.push(None::<&str>, I::I32Add);
    f.push(None::<&str>, I::LocalGet(GE));
    f.push(None::<&str>, I::Or);
    f.push(None::<&str>, I::LocalSet(Q0));

    // count -= 1; loop back to the test.
    f.push(None::<&str>, I::LocalGet(CNT));
    f.push(None::<&str>, I::ConstI32(1));
    f.push(None::<&str>, I::I32Sub);
    f.push(None::<&str>, I::LocalSet(CNT));
    f.push(None::<&str>, I::Jump("chk".to_string()));

    // Done: return (r_lo, r_hi, q_lo, q_hi) — quotient pair on top.
    f.push(Some("done"), I::LocalGet(R0));
    f.push(None::<&str>, I::LocalGet(R1));
    f.push(None::<&str>, I::LocalGet(Q0));
    f.push(None::<&str>, I::LocalGet(Q1));
    f.push(None::<&str>, I::Return);
    f
}

/// Build one program-level software shift helper.
///
/// Signature: takes `(lo, hi, amount)` on the operand stack (amount on top,
/// matching the `param_cells = 3` prologue convention) and returns the shifted
/// `(lo', hi')` with the high limb on top (`return_cells = 2`). The helper
/// steps one bit per loop iteration so a shift needs no 32-bit ISA primitive
/// beyond word shifts and is exact for any amount in `[0, 64)`. The caller
/// masks amounts to `& 63` before calling.
///
/// Loop layout (labels are function-local):
/// ```text
/// prologue: LocalSet 2, 1, 0        // lo slot0, hi slot1, amount slot2
/// chk:      amount == 0 ? done : step
/// step:     amount -= 1; apply one width-bit shift; goto chk
/// done:     push lo; push hi; return
/// ```
fn build_shift_helper(name: &str) -> Function {
    let mut f = Function::new(name);
    f.param_cells = 3;
    f.local_count = 3; // lo slot0, hi slot1, amount slot2
    f.return_cells = 2;

    // Prologue: pop (lo, hi, amount) from the operand stack into slots.
    for slot in (0..3).rev() {
        f.push(None::<&str>, IsaInstr::LocalSet(slot));
    }

    // Loop test.
    f.push(Some("chk"), IsaInstr::LocalGet(2));
    f.push(None::<&str>, IsaInstr::ConstI32(0));
    f.push(None::<&str>, IsaInstr::Eq);
    f.push(None::<&str>, IsaInstr::Branch("done".to_string(), "step".to_string()));

    // Loop body (one bit).
    f.push(Some("step"), IsaInstr::LocalGet(2));
    f.push(None::<&str>, IsaInstr::ConstI32(1));
    f.push(None::<&str>, IsaInstr::I32Sub);
    f.push(None::<&str>, IsaInstr::LocalSet(2));
    match name {
        "__sair_shl64" => {
            // hi' = (hi << 1) | (lo >> 31); then lo' = lo << 1.
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::Dup);
            f.push(None::<&str>, IsaInstr::I32Add);
            f.push(None::<&str>, IsaInstr::LocalGet(0));
            f.push(None::<&str>, IsaInstr::ConstI32(31));
            f.push(None::<&str>, IsaInstr::Shr);
            f.push(None::<&str>, IsaInstr::Or);
            f.push(None::<&str>, IsaInstr::LocalSet(1));
            f.push(None::<&str>, IsaInstr::LocalGet(0));
            f.push(None::<&str>, IsaInstr::Dup);
            f.push(None::<&str>, IsaInstr::I32Add);
            f.push(None::<&str>, IsaInstr::LocalSet(0));
        }
        "__sair_lshr64" => {
            // lo' = (lo >> 1) | (hi << 31); then hi' = hi >> 1.
            f.push(None::<&str>, IsaInstr::LocalGet(0));
            f.push(None::<&str>, IsaInstr::ConstI32(1));
            f.push(None::<&str>, IsaInstr::Shr);
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::ConstI32(31));
            f.push(None::<&str>, IsaInstr::Shl);
            f.push(None::<&str>, IsaInstr::Or);
            f.push(None::<&str>, IsaInstr::LocalSet(0));
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::ConstI32(1));
            f.push(None::<&str>, IsaInstr::Shr);
            f.push(None::<&str>, IsaInstr::LocalSet(1));
        }
        "__sair_ashr64" => {
            // lo' = (lo >> 1) | (hi << 31); hi' = (hi >> 1) | (hi & 0x8000_0000)
            // (arithmetic right shift by one replicates the sign bit).
            f.push(None::<&str>, IsaInstr::LocalGet(0));
            f.push(None::<&str>, IsaInstr::ConstI32(1));
            f.push(None::<&str>, IsaInstr::Shr);
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::ConstI32(31));
            f.push(None::<&str>, IsaInstr::Shl);
            f.push(None::<&str>, IsaInstr::Or);
            f.push(None::<&str>, IsaInstr::LocalSet(0));
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::ConstI32(1));
            f.push(None::<&str>, IsaInstr::Shr);
            f.push(None::<&str>, IsaInstr::LocalGet(1));
            f.push(None::<&str>, IsaInstr::ConstI32(0x8000_0000));
            f.push(None::<&str>, IsaInstr::And);
            f.push(None::<&str>, IsaInstr::Or);
            f.push(None::<&str>, IsaInstr::LocalSet(1));
        }
        _ => unreachable!("unknown shift helper: {name}"),
    }
    f.push(None::<&str>, IsaInstr::Jump("chk".to_string()));

    // Done: return (lo', hi').
    f.push(Some("done"), IsaInstr::LocalGet(0));
    f.push(None::<&str>, IsaInstr::LocalGet(1));
    f.push(None::<&str>, IsaInstr::Return);
    f
}

/// Per-function lowering context: slot allocation and value metadata.
struct LowerCtx {
    /// First slot index for each value.
    first_slot: HashMap<ValueId, u32>,
    /// Cell count for each value.
    value_cells: HashMap<ValueId, u32>,
    /// Total number of local slots (including parameters and the temp slot).
    local_count: u32,
    /// Slot reserved for phi cycle breaking.
    temp_slot: u32,
    /// Number of parameter cells.
    param_cells: u32,
    /// Number of return cells.
    return_cells: u32,
}

impl LowerCtx {
    fn new(profile: &TargetProfile, func: &IrFunction) -> Result<Self, LowerError> {
        let mut ctx = LowerCtx {
            first_slot: HashMap::new(),
            value_cells: HashMap::new(),
            local_count: 0,
            temp_slot: 0,
            param_cells: 0,
            return_cells: Self::cell_count_for_type(profile, &func.return_ty),
        };

        // Allocate slots for every SSA value in id order (params first, then
        // result-producing instructions in block order).
        for (id, value) in func.values.iter().enumerate() {
            let cells = Self::cell_count_for_type(profile, &value.ty);
            ctx.first_slot.insert(id, ctx.local_count);
            ctx.value_cells.insert(id, cells);
            ctx.local_count += cells;
        }

        ctx.param_cells = func
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| ctx.value_cells.get(&i).copied().unwrap_or(0))
            .sum();

        ctx.temp_slot = ctx.local_count;
        ctx.local_count += 1;
        Ok(ctx)
    }

    fn cell_count_for_type(profile: &TargetProfile, ty: &IrType) -> u32 {
        if ty.is_void() {
            0
        } else {
            profile.cells_for_type(ty.size_in_bytes() * 8)
        }
    }

    fn cells(&self, value: ValueId) -> Result<u32, LowerError> {
        self.value_cells
            .get(&value)
            .copied()
            .ok_or_else(|| LowerError::ValidationError(format!("undefined value %{value}")))
    }

    fn first_slot(&self, value: ValueId) -> Result<u32, LowerError> {
        self.first_slot
            .get(&value)
            .copied()
            .ok_or_else(|| LowerError::ValidationError(format!("undefined value %{value}")))
    }
}

/// Helper that applies the pending block label to the next emitted instruction
/// and then clears it.
struct FuncEmitter<'a> {
    func: &'a mut Function,
    pending_label: Option<String>,
}

impl<'a> FuncEmitter<'a> {
    fn new(func: &'a mut Function) -> Self {
        FuncEmitter {
            func,
            pending_label: None,
        }
    }

    fn set_label(&mut self, label: &str) {
        self.pending_label = Some(label.to_string());
    }

    fn push(&mut self, instr: IsaInstr) {
        let label = self.pending_label.take();
        self.func.push(label, instr);
    }
}

#[derive(Debug, Clone)]
enum PhiSrc {
    /// Copy from an SSA value's slot.
    Slot(u32),
    /// Rematerialize a constant operand directly at the copy site. Constants
    /// that feed a phi are defined by a `Const` instruction in the *successor*
    /// block (that is where the translator emits them), so on the VM the slot
    /// holding them is only initialized after the successor's own code runs —
    /// too late for copies emitted at the end of the predecessor (first loop
    /// iteration reads an uninitialized slot). Rematerializing the immediate at
    /// the edge makes the copy independent of block order.
    Const(Constant),
}

#[derive(Debug, Clone)]
struct Copy {
    dst: u32,
    src: PhiSrc,
}

enum CopyPlacement {
    AtStartOf(String),
    AtEndOf(String),
}

fn copy_placement(
    func: &IrFunction,
    trampolines: &HashSet<String>,
    pred_label: &str,
    succ_label: &str,
) -> CopyPlacement {
    // If the predecessor is a trampoline block, copies live at its start.
    if trampolines.contains(pred_label) {
        return CopyPlacement::AtStartOf(pred_label.to_string());
    }

    let pred = func
        .blocks
        .iter()
        .find(|b| b.label == pred_label)
        .expect("predecessor block missing");

    match &pred.terminator {
        Terminator::Branch { target } if target == succ_label => {
            CopyPlacement::AtEndOf(pred_label.to_string())
        }
        Terminator::CondBranch { .. } => {
            // Conditional branch to a single-predecessor successor: copies go
            // at the start of the successor. If the successor had multiple
            // predecessors the edge would have been split into a trampoline.
            CopyPlacement::AtStartOf(succ_label.to_string())
        }
        _ => CopyPlacement::AtStartOf(succ_label.to_string()),
    }
}

fn collect_phi_copies(
    func: &IrFunction,
    ctx: &LowerCtx,
) -> Result<HashMap<(String, String), Vec<Copy>>, LowerError> {
    let mut per_edge: HashMap<(String, String), Vec<Copy>> = HashMap::new();

    // Map every value id that is a constant instruction back to its constant,
    // so a phi operand can be rematerialized at the edge instead of copied
    // from the successor-local slot (see PhiSrc::Const).
    let mut next_const_id = func.params.len();
    let mut const_at: HashMap<ValueId, Constant> = HashMap::new();
    for block in &func.blocks {
        for instr in &block.instructions {
            if instr.result_type().is_some() {
                if let SairInstr::Const(c) = instr {
                    const_at.insert(next_const_id, c.clone());
                }
                next_const_id += 1;
            }
        }
    }

    for succ in &func.blocks {
        for (instr_idx, instr) in succ.instructions.iter().enumerate() {
            if let SairInstr::Phi { incoming, .. } = instr {
                // The phi result id equals the instruction's result id.
                let phi_result = find_result_id(func, &succ.label, instr_idx);
                let phi_first = ctx.first_slot(phi_result)?;
                let phi_cells = ctx.cells(phi_result)?;

                for (value, pred_label) in incoming {
                    let cells = ctx.cells(*value)?;
                    if phi_cells != cells {
                        return Err(LowerError::ValidationError(format!(
                            "phi operand cell count mismatch: phi {} cells, operand {} cells",
                            phi_cells, cells
                        )));
                    }
                    if let Some(c) = const_at.get(value) {
                        // Constants feeding a phi are rematerialized at the edge
                        // (see `PhiSrc::Const`). A multi-cell constant (i64) is
                        // split into one imm per limb so each copy is a single
                        // cell write, matching the layout of a slot-moved value.
                        for limb in 0..cells {
                            per_edge
                                .entry((pred_label.clone(), succ.label.clone()))
                                .or_default()
                                .push(Copy {
                                    dst: phi_first + limb,
                                    src: PhiSrc::Const(const_limb(c, limb, cells)),
                                });
                        }
                        continue;
                    }
                    let src_first = ctx.first_slot(*value)?;
                    for offset in 0..phi_cells {
                        per_edge
                            .entry((pred_label.clone(), succ.label.clone()))
                            .or_default()
                            .push(Copy {
                                dst: phi_first + offset,
                                src: PhiSrc::Slot(src_first + offset),
                            });
                    }
                }
            }
        }
    }

    Ok(per_edge)
}

fn find_result_id(func: &IrFunction, block_label: &str, instr_idx: usize) -> ValueId {
    let mut id = func.params.len();
    for block in &func.blocks {
        for (idx, instr) in block.instructions.iter().enumerate() {
            if block.label == block_label && idx == instr_idx {
                return id;
            }
            if instr.result_type().is_some() {
                id += 1;
            }
        }
    }
    panic!("result id not found for {}:{}", block_label, instr_idx);
}

fn emit_copies(copies: &[Copy], temp_slot: u32, f: &mut FuncEmitter<'_>) {
    // Immediate (constant) writes carry no source dependency, but a slot move
    // in the same batch may read the slot an immediate is about to write — that
    // read must observe the pre-edge (old) value. So slot moves are emitted
    // first under parallel-copy ordering, and constant writes last.
    let mut slot_moves: Vec<(u32, u32)> = Vec::new();
    let mut imm_writes: Vec<&Copy> = Vec::new();
    for c in copies {
        match &c.src {
            PhiSrc::Slot(src) => slot_moves.push((c.dst, *src)),
            PhiSrc::Const(_) => imm_writes.push(c),
        }
    }

    for (src, dst) in resolve_parallel_copies(&slot_moves, temp_slot) {
        f.push(IsaInstr::LocalGet(src));
        f.push(IsaInstr::LocalSet(dst));
    }
    for c in imm_writes {
        let (instr, dst) = match &c.src {
            PhiSrc::Const(value) => (const_push_isa(value), c.dst),
            PhiSrc::Slot(_) => unreachable!(),
        };
        f.push(instr);
        f.push(IsaInstr::LocalSet(dst));
    }
}

/// The ISA instruction that pushes `c` onto the operand stack as a typed value.
/// Mirrors `IsaLowerer::emit_const`; sub-32-bit integers travel as I32 cells.
/// Only single-cell constants reach this helper — a multi-cell constant is split
/// into per-limb `Const` copies by [`const_limb`] before emission.
fn const_push_isa(c: &Constant) -> IsaInstr {
    match c {
        Constant::I32(v) => IsaInstr::ConstI32(*v),
        Constant::F64(v) => IsaInstr::ConstF64(*v),
        Constant::I1(v) => IsaInstr::ConstI1(*v),
        Constant::I8(v) => IsaInstr::ConstI32(*v as u32),
        Constant::I16(v) => IsaInstr::ConstI32(*v as u32),
        Constant::I64(_) => unreachable!(
            "i64 phi constants are split into limb copies by const_limb before emission"
        ),
    }
}

/// Extract the `limb`-th single-cell constant of a `cells`-cell constant. A
/// one-cell constant is returned unchanged; an i64 constant yields its low
/// (limb 0) or high (limb 1) 32-bit half.
fn const_limb(c: &Constant, limb: u32, cells: u32) -> Constant {
    if cells == 1 {
        return c.clone();
    }
    match c {
        Constant::I64(v) => Constant::I32((v >> (32 * limb)) as u32),
        _ => unreachable!("multi-cell constant must be i64"),
    }
}

fn resolve_parallel_copies(copies: &[(u32, u32)], temp_slot: u32) -> Vec<(u32, u32)> {
    let mut map: HashMap<u32, u32> = copies
        .iter()
        .filter(|(dst, src)| dst != src)
        .map(|(dst, src)| (*dst, *src))
        .collect();
    let mut ordered = Vec::new();

    while !map.is_empty() {
        let sources: HashSet<u32> = map.values().copied().collect();
        let leaves: Vec<u32> = map
            .keys()
            .filter(|k| !sources.contains(k))
            .copied()
            .collect();
        if !leaves.is_empty() {
            for dst in leaves {
                let src = map.remove(&dst).unwrap();
                ordered.push((src, dst));
            }
            continue;
        }

        // Only cycles remain. Break one cycle using the temp slot.
        let start = *map.keys().next().unwrap();
        let mut walk: Vec<(u32, u32)> = Vec::new();
        let mut visited = HashSet::new();
        let mut cur = start;
        loop {
            if !visited.insert(cur) {
                break;
            }
            let src = *map.get(&cur).unwrap();
            walk.push((cur, src));
            cur = src;
        }
        let repeat_pos = walk.iter().position(|(d, _)| *d == cur).unwrap();
        let cycle: Vec<(u32, u32)> = walk.split_off(repeat_pos);

        if cycle.is_empty() {
            continue;
        }

        let n = cycle.len();
        let saved = cycle[0].0;
        ordered.push((saved, temp_slot));
        for &(dst, src) in cycle.iter().take(n - 1) {
            ordered.push((src, dst));
        }
        let last_dst = cycle[n - 1].0;
        ordered.push((temp_slot, last_dst));

        for (dst, _) in &cycle {
            map.remove(dst);
        }
    }

    ordered
}

fn split_critical_edges(func: &mut IrFunction) -> HashSet<String> {
    let pred_map = func.build_pred_map();
    let succ_map = build_succ_map(func);
    let mut trampoline_labels = HashSet::new();
    let mut additions: Vec<(usize, BasicBlock)> = Vec::new();
    let mut terminator_updates: Vec<(usize, Terminator)> = Vec::new();
    let mut phi_label_updates: Vec<(String, String, String)> = Vec::new();

    for (pred_idx, pred) in func.blocks.iter().enumerate() {
        let pred_succs = succ_map.get(&pred.label).cloned().unwrap_or_default();
        if pred_succs.len() <= 1 {
            continue;
        }
        for succ_label in pred_succs {
            let succ_preds = pred_map.get(&succ_label).cloned().unwrap_or_default();
            if succ_preds.len() <= 1 {
                continue;
            }
            let trampoline_label = format!("{}_{}_trampoline", pred.label, succ_label);
            trampoline_labels.insert(trampoline_label.clone());
            phi_label_updates.push((pred.label.clone(), succ_label.clone(), trampoline_label.clone()));
            additions.push((
                pred_idx + 1,
                BasicBlock {
                    label: trampoline_label.clone(),
                    instructions: Vec::new(),
                    debug_locs: Vec::new(),
                    terminator: Terminator::Branch { target: succ_label.clone() },
                },
            ));

            let new_term = match &pred.terminator {
                Terminator::CondBranch {
                    condition,
                    true_target,
                    false_target,
                } => {
                    let condition = *condition;
                    let mut true_target = true_target.clone();
                    let mut false_target = false_target.clone();
                    if true_target == succ_label {
                        true_target = trampoline_label.clone();
                    }
                    if false_target == succ_label {
                        false_target = trampoline_label.clone();
                    }
                    Terminator::CondBranch {
                        condition,
                        true_target,
                        false_target,
                    }
                }
                Terminator::Branch { target: _ } => Terminator::Branch {
                    target: trampoline_label.clone(),
                },
                other => other.clone(),
            };
            terminator_updates.push((pred_idx, new_term));
        }
    }

    for (idx, term) in terminator_updates {
        func.blocks[idx].terminator = term;
    }

    // Insert trampoline blocks in reverse order so earlier indices stay valid.
    additions.sort_by_key(|(idx, _)| *idx);
    for (insert_offset, (insert_idx, block)) in additions.into_iter().enumerate() {
        func.blocks.insert(insert_idx + insert_offset, block);
    }

    for (pred_label, succ_label, trampoline_label) in phi_label_updates {
        if let Some(succ_idx) = func.block_index(&succ_label) {
            let succ = &mut func.blocks[succ_idx];
            for instr in &mut succ.instructions {
                if let SairInstr::Phi { incoming, .. } = instr {
                    for (_, pred) in incoming.iter_mut() {
                        if *pred == pred_label {
                            *pred = trampoline_label.clone();
                        }
                    }
                }
            }
        }
    }

    trampoline_labels
}

fn build_succ_map(func: &IrFunction) -> HashMap<String, Vec<String>> {
    let mut succs: HashMap<String, Vec<String>> = HashMap::new();
    for block in &func.blocks {
        let labels = block.referenced_labels();
        succs.insert(block.label.clone(), labels);
    }
    succs
}
