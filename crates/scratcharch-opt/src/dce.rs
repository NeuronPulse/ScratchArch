use std::collections::HashSet;
use crate::analysis::FunctionAnalysis;
use crate::pass::OptimizationPass;
use crate::util::compact_value_ids;
use scratcharch_ir::instruction::Instruction;
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::value::ValueId;

pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str {
        "dce"
    }

    fn run(&mut self, module: &mut IrModule) {
        for i in 0..module.functions.len() {
            let _count = dce_function(&mut module.functions[i]);
        }
    }
}

fn dce_function(func: &mut scratcharch_ir::function::IrFunction) -> usize {
    let mut total_removed = 0;

    loop {
        let analysis = FunctionAnalysis::build(func);
        let mut dead: HashSet<ValueId> = HashSet::new();
        let mut cur_id = func.params.len();

        for block in &func.blocks {
            for instr in &block.instructions {
                if instr.result_type().is_none() {
                    continue;
                }

                if is_safe_to_remove(instr) && !analysis.has_users(cur_id) {
                    dead.insert(cur_id);
                }

                cur_id += 1;
            }
        }

        if dead.is_empty() {
            return total_removed;
        }

        // Compact ids first while the dead instructions are still present so
        // that the old->new id mapping is computed against the original ids.
        // Then drop the dead instructions.
        compact_value_ids(func, &dead);
        remove_dead_instructions(func, &dead);
        total_removed += dead.len();
    }
}

fn remove_dead_instructions(func: &mut scratcharch_ir::function::IrFunction, dead: &HashSet<ValueId>) {
    let mut cur_id = func.params.len();
    for block in &mut func.blocks {
        block.instructions.retain(|instr| {
            if instr.result_type().is_some() {
                let id = cur_id;
                cur_id += 1;
                !dead.contains(&id)
            } else {
                true
            }
        });
    }
}

fn is_safe_to_remove(instr: &Instruction) -> bool {
    match instr {
        Instruction::Add { .. }
        | Instruction::Sub { .. }
        | Instruction::Mul { .. }
        | Instruction::Eq { .. }
        | Instruction::Lt { .. }
        | Instruction::Gt { .. }
        | Instruction::Const(_) => true,
        Instruction::Div { .. }
        | Instruction::Rem { .. }
        | Instruction::Alloca { .. }
        | Instruction::Load { .. }
        | Instruction::Store { .. }
        | Instruction::Call { .. }
        | Instruction::Phi { .. }
        | Instruction::Gep { .. } => false,
    }
}
