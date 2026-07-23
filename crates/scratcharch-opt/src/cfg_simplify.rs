use std::collections::HashSet;
use crate::analysis::FunctionAnalysis;
use crate::pass::OptimizationPass;
use crate::util::compact_value_ids;
use scratcharch_ir::instruction::{Instruction, Terminator};
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::value::ValueId;

pub struct CfgSimplify;

impl OptimizationPass for CfgSimplify {
    fn name(&self) -> &str {
        "cfg-simplify"
    }

    fn run(&mut self, module: &mut IrModule) {
        for i in 0..module.functions.len() {
            simplify_function(&mut module.functions[i]);
        }
    }
}

fn simplify_function(func: &mut scratcharch_ir::function::IrFunction) -> usize {
    let mut total = 0;
    loop {
        let removed = remove_unreachable_blocks(func);
        let merged = merge_trivial_blocks(func);
        total += removed + merged;
        if removed == 0 && merged == 0 {
            break;
        }
    }
    total
}

fn remove_unreachable_blocks(func: &mut scratcharch_ir::function::IrFunction) -> usize {
    let analysis = FunctionAnalysis::build(func);
    let reachable: HashSet<String> = analysis.reachable_blocks().clone();

    let before = func.blocks.len();
    if before == 0 {
        return 0;
    }

    // Collect result ids that belong to blocks we are about to delete.
    let mut removed_ids: HashSet<ValueId> = HashSet::new();
    let mut cur_id = func.params.len();
    for block in &func.blocks {
        let block_reachable = reachable.contains(&block.label);
        for instr in &block.instructions {
            if instr.result_type().is_some() {
                if !block_reachable {
                    removed_ids.insert(cur_id);
                }
                cur_id += 1;
            }
        }
    }

    if removed_ids.is_empty() {
        return 0;
    }

    // Compact value ids while the unreachable blocks are still present so
    // the original ids of deleted results can be skipped correctly.
    compact_value_ids(func, &removed_ids);

    func.blocks.retain(|b| reachable.contains(&b.label));
    let removed = before - func.blocks.len();

    // Remove phi incoming edges that came from deleted, unreachable blocks.
    for block in &mut func.blocks {
        for instr in &mut block.instructions {
            if let Instruction::Phi { incoming, .. } = instr {
                incoming.retain(|(_, label)| reachable.contains(label));
            }
        }
    }

    removed
}

fn merge_trivial_blocks(func: &mut scratcharch_ir::function::IrFunction) -> usize {
    let mut total = 0;
    let mut skipped: HashSet<String> = HashSet::new();

    loop {
        let analysis = FunctionAnalysis::build(func);
        let candidate = find_trivial_branch_block(func, &analysis, &skipped);

        let (src_idx, dst_label) = match candidate {
            Some(pair) => pair,
            None => return total,
        };

        let src_label = func.blocks[src_idx].label.clone();

        // Never collapse a block into itself.
        if dst_label == src_label {
            return total;
        }

        let preds: Vec<String> = analysis
            .predecessors(&src_label)
            .cloned()
            .unwrap_or_default();

        // A trivial block with no predecessors is either the entry block or
        // unreachable. Skip it and keep searching for a mergeable candidate.
        if preds.is_empty() {
            skipped.insert(src_label);
            continue;
        }

        // If the destination has phi nodes, changing predecessor edges without
        // updating phi incoming values would break SSA semantics. Skip and keep
        // searching.
        if block_has_phi(func, &dst_label) {
            skipped.insert(src_label);
            continue;
        }

        redirect_all_predecessors(func, &preds, &src_label, &dst_label);
        func.blocks.retain(|b| b.label != src_label);
        total += 1;
        skipped.clear();
    }
}

fn block_has_phi(func: &scratcharch_ir::function::IrFunction, label: &str) -> bool {
    func.blocks
        .iter()
        .find(|b| b.label == label)
        .is_some_and(|b| {
            b.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Phi { .. }))
        })
}

fn find_trivial_branch_block(
    func: &scratcharch_ir::function::IrFunction,
    analysis: &FunctionAnalysis,
    skipped: &HashSet<String>,
) -> Option<(usize, String)> {
    for (i, block) in func.blocks.iter().enumerate() {
        if skipped.contains(&block.label) {
            continue;
        }
        if !block.instructions.is_empty() {
            continue;
        }
        if let Terminator::Branch { target } = &block.terminator {
            // Only consider blocks that have predecessors. Blocks without
            // predecessors cannot be safely bypassed (they are the entry or
            // unreachable).
            let preds = analysis.predecessors(&block.label);
            if preds.is_none_or(|p| p.is_empty()) {
                continue;
            }
            return Some((i, target.clone()));
        }
    }
    None
}

fn redirect_all_predecessors(
    func: &mut scratcharch_ir::function::IrFunction,
    preds: &[String],
    src_label: &str,
    dst_label: &str,
) {
    for pred_label in preds {
        if let Some(pred_block) = func.blocks.iter_mut().find(|b| b.label == *pred_label) {
            redirect_branch(&mut pred_block.terminator, src_label, dst_label);
        }
    }
}

fn redirect_branch(term: &mut Terminator, from: &str, to: &str) {
    match term {
        Terminator::Branch { target } if target == from => {
            *target = to.to_string();
        }
        Terminator::CondBranch {
            true_target,
            false_target,
            ..
        } => {
            if true_target == from {
                *true_target = to.to_string();
            }
            if false_target == from {
                *false_target = to.to_string();
            }
        }
        _ => {}
    }
}
