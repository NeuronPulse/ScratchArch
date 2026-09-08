use std::collections::{HashMap, HashSet};
use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{GepIndex, Instruction, Terminator};
use scratcharch_ir::value::{SsaValue, ValueId};

/// Remove a set of dead result-value ids from a function and renumber the
/// remaining SSA ids so that all operands still refer to the correct values.
///
/// This must be called after deleting instructions (or whole blocks) whose
/// results had ids in `removed_ids`. Parameters are never removed and keep
/// their original ids.
pub fn compact_value_ids(func: &mut IrFunction, removed_ids: &HashSet<ValueId>) {
    if removed_ids.is_empty() {
        return;
    }

    let param_count = func.params.len();
    let mut old_to_new: HashMap<ValueId, ValueId> = HashMap::with_capacity(func.value_counter());
    for i in 0..param_count {
        old_to_new.insert(i, i);
    }

    let mut next_id = param_count;
    let mut cur_id = param_count;

    for block in &func.blocks {
        for instr in &block.instructions {
            if instr.result_type().is_some() {
                if !removed_ids.contains(&cur_id) {
                    old_to_new.insert(cur_id, next_id);
                    next_id += 1;
                }
                cur_id += 1;
            }
        }
    }

    // Rewrite every SSA value operand in the remaining IR.
    for block in &mut func.blocks {
        for instr in &mut block.instructions {
            remap_instruction(instr, &old_to_new);
        }
        remap_terminator(&mut block.terminator, &old_to_new);
    }

    // Rebuild the function's value table so indices match the new ids.
    let mut new_values: Vec<SsaValue> = Vec::with_capacity(next_id);
    for (old_id, value) in func.values.iter().enumerate() {
        if let Some(&new_id) = old_to_new.get(&old_id) {
            new_values.push(SsaValue::new(new_id, value.ty, value.name.clone()));
        }
    }
    new_values.sort_by_key(|v| v.id);
    func.values = new_values;
    func.set_value_counter(next_id);
}

fn remap_id(map: &HashMap<ValueId, ValueId>, id: ValueId) -> ValueId {
    map.get(&id).copied().unwrap_or(id)
}

fn remap_instruction(instr: &mut Instruction, map: &HashMap<ValueId, ValueId>) {
    match instr {
        Instruction::Add { lhs, rhs, .. }
        | Instruction::Sub { lhs, rhs, .. }
        | Instruction::Mul { lhs, rhs, .. }
        | Instruction::Div { lhs, rhs, .. }
        | Instruction::Rem { lhs, rhs, .. }
        | Instruction::Eq { lhs, rhs, .. }
        | Instruction::Lt { lhs, rhs, .. }
        | Instruction::Gt { lhs, rhs, .. } => {
            *lhs = remap_id(map, *lhs);
            *rhs = remap_id(map, *rhs);
        }
        Instruction::Const(_) => {}
        Instruction::Alloca { .. } => {}
        Instruction::Load { addr, .. } => {
            *addr = remap_id(map, *addr);
        }
        Instruction::Store { value, addr, .. } => {
            *value = remap_id(map, *value);
            *addr = remap_id(map, *addr);
        }
        Instruction::Call { args, .. } => {
            for arg in args {
                *arg = remap_id(map, *arg);
            }
        }
        Instruction::Phi { incoming, .. } => {
            for (value, _label) in incoming {
                *value = remap_id(map, *value);
            }
        }
        Instruction::Cast { value, .. } => {
            *value = remap_id(map, *value);
        }
        Instruction::Select { condition, then_value, else_value, .. } => {
            *condition = remap_id(map, *condition);
            *then_value = remap_id(map, *then_value);
            *else_value = remap_id(map, *else_value);
        }
        Instruction::Gep { base, indices, .. } => {
            *base = remap_id(map, *base);
            for index in indices {
                if let GepIndex::Dynamic(id) = index {
                    *id = remap_id(map, *id);
                }
            }
        }
    }
}

fn remap_terminator(term: &mut Terminator, map: &HashMap<ValueId, ValueId>) {
    match term {
        Terminator::Branch { .. } => {}
        Terminator::CondBranch { condition, .. } => {
            *condition = remap_id(map, *condition);
        }
        Terminator::Return { value } => {
            if let Some(id) = value {
                *id = remap_id(map, *id);
            }
        }
        Terminator::Unreachable => {}
    }
}
