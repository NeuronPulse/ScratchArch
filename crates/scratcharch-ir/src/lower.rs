use std::collections::{HashMap, HashSet};

use scratcharch_core::instruction::Instruction as IsaInstr;
use scratcharch_core::program::{Function, Program};
use scratcharch_target::profile::TargetProfile;

use crate::block::BasicBlock;
use crate::function::IrFunction;
use crate::instruction::{GepIndex, Instruction as SairInstr, Terminator};
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
                self.lower_instr(instr, result_id, &ctx, &mut emitter)?;
            }

            // Emit outgoing phi copies before the terminator.
            if let Some(copies) = end_copies.get(&block.label) {
                emit_copies(copies, ctx.temp_slot, &mut emitter);
            }

            self.lower_term(&block.terminator, &ctx, &mut emitter);
        }

        Ok(f)
    }

    fn lower_instr(
        &self,
        instr: &SairInstr,
        result_id: Option<ValueId>,
        ctx: &LowerCtx,
        f: &mut FuncEmitter<'_>,
    ) -> Result<(), LowerError> {
        match instr {
            SairInstr::Add { ty, lhs, rhs }
            | SairInstr::Sub { ty, lhs, rhs }
            | SairInstr::Mul { ty, lhs, rhs }
            | SairInstr::Div { ty, lhs, rhs }
            | SairInstr::Rem { ty, lhs, rhs }
            | SairInstr::Eq { ty, lhs, rhs }
            | SairInstr::Lt { ty, lhs, rhs }
            | SairInstr::Gt { ty, lhs, rhs } => {
                self.check_single_cell(*ty)?;
                self.emit_load(*lhs, ctx, f)?;
                self.emit_load(*rhs, ctx, f)?;
                let isa_op = match instr {
                    SairInstr::Add { .. } => IsaInstr::I32Add,
                    SairInstr::Sub { .. } => IsaInstr::I32Sub,
                    SairInstr::Mul { .. } => IsaInstr::I32Mul,
                    SairInstr::Div { .. } => IsaInstr::I32Div,
                    SairInstr::Rem { .. } => IsaInstr::I32Rem,
                    SairInstr::Eq { .. } => IsaInstr::Eq,
                    SairInstr::Lt { .. } => IsaInstr::Lt,
                    SairInstr::Gt { .. } => IsaInstr::Gt,
                    _ => unreachable!(),
                };
                f.push(isa_op);
                if let Some(id) = result_id {
                    self.emit_store(id, ctx, f)?;
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
                self.check_single_cell(*ty)?;
                self.emit_load(*addr, ctx, f)?;
                f.push(IsaInstr::Load);
                if let Some(id) = result_id {
                    self.emit_store(id, ctx, f)?;
                }
            }
            SairInstr::Store { ty: _, value, addr } => {
                // The VM expects the value on top of the operand stack and the
                // address underneath it, so emit the address first.
                self.emit_load(*addr, ctx, f)?;
                self.emit_load(*value, ctx, f)?;
                f.push(IsaInstr::Store);
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
            SairInstr::Phi { .. } => {
                // Phis are lowered via edge copies; the instruction itself is a no-op.
            }
        }
        Ok(())
    }

    fn lower_term(&self, term: &Terminator, ctx: &LowerCtx, f: &mut FuncEmitter<'_>) {
        match term {
            Terminator::Branch { target } => {
                f.push(IsaInstr::Jump(target.clone()));
            }
            Terminator::CondBranch {
                condition,
                true_target,
                false_target,
            } => {
                self.emit_load(*condition, ctx, f).expect("cond branch condition value missing");
                f.push(IsaInstr::Branch(true_target.clone(), false_target.clone()));
            }
            Terminator::Return { value } => {
                if let Some(v) = value {
                    self.emit_load(*v, ctx, f).expect("return value missing");
                }
                f.push(IsaInstr::Return);
            }
        }
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
        }
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
                    self.emit_load(*value, ctx, f)?;
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

    fn check_single_cell(&self, ty: IrType) -> Result<(), LowerError> {
        let cells = LowerCtx::cell_count_for_type(&self.profile, &ty);
        if cells != 1 {
            return Err(LowerError::UnsupportedType(format!(
                "multi-cell value of type {ty} ({cells} cells) not yet executable",
            )));
        }
        Ok(())
    }
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

#[derive(Debug, Clone, Copy)]
struct Copy {
    dst: u32,
    src: u32,
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

    for succ in &func.blocks {
        for (instr_idx, instr) in succ.instructions.iter().enumerate() {
            if let SairInstr::Phi { incoming, .. } = instr {
                // The phi result id equals the instruction's result id.
                let phi_result = find_result_id(func, &succ.label, instr_idx);
                let phi_first = ctx.first_slot(phi_result)?;
                let phi_cells = ctx.cells(phi_result)?;

                for (value, pred_label) in incoming {
                    let src_first = ctx.first_slot(*value)?;
                    let src_cells = ctx.cells(*value)?;
                    if phi_cells != src_cells {
                        return Err(LowerError::ValidationError(format!(
                            "phi operand cell count mismatch: phi {} cells, operand {} cells",
                            phi_cells, src_cells
                        )));
                    }
                    for offset in 0..phi_cells {
                        per_edge
                            .entry((pred_label.clone(), succ.label.clone()))
                            .or_default()
                            .push(Copy {
                                dst: phi_first + offset,
                                src: src_first + offset,
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
    let ordered = resolve_parallel_copies(copies, temp_slot);
    for (src, dst) in ordered {
        f.push(IsaInstr::LocalGet(src));
        f.push(IsaInstr::LocalSet(dst));
    }
}

fn resolve_parallel_copies(copies: &[Copy], temp_slot: u32) -> Vec<(u32, u32)> {
    let mut map: HashMap<u32, u32> = copies
        .iter()
        .filter(|c| c.dst != c.src)
        .map(|c| (c.dst, c.src))
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
