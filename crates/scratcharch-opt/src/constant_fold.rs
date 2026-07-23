use crate::analysis::FunctionAnalysis;
use crate::pass::OptimizationPass;
use scratcharch_ir::instruction::Instruction;
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::value::Constant;

pub struct ConstantFold;

impl OptimizationPass for ConstantFold {
    fn name(&self) -> &str {
        "constant-fold"
    }

    fn run(&mut self, module: &mut IrModule) {
        for i in 0..module.functions.len() {
            fold_function(&mut module.functions[i]);
        }
    }
}

fn fold_function(func: &mut scratcharch_ir::function::IrFunction) -> usize {
    let mut total = 0;

    loop {
        let analysis = FunctionAnalysis::build(func);
        let mut folded = 0;

        for block in &mut func.blocks {
            for instr in &mut block.instructions {
                if try_fold(instr, &analysis) {
                    folded += 1;
                }
            }
        }

        if folded == 0 {
            return total;
        }

        total += folded;
    }
}

fn try_fold(
    instr: &mut Instruction,
    analysis: &FunctionAnalysis,
) -> bool {
    let (lhs, rhs) = match instr {
        Instruction::Add { lhs, rhs, .. }
        | Instruction::Sub { lhs, rhs, .. }
        | Instruction::Mul { lhs, rhs, .. }
        | Instruction::Eq { lhs, rhs, .. }
        | Instruction::Lt { lhs, rhs, .. }
        | Instruction::Gt { lhs, rhs, .. } => (*lhs, *rhs),
        _ => return false,
    };

    let lhs_val = analysis.get_constant(lhs);
    let rhs_val = analysis.get_constant(rhs);
    let (lhs_c, rhs_c) = match (lhs_val, rhs_val) {
        (Some(l), Some(r)) => (l.clone(), r.clone()),
        _ => return false,
    };

    let folded = match instr {
        Instruction::Add { .. } => fold_binary(|a, b| a.wrapping_add(b), &lhs_c, &rhs_c),
        Instruction::Sub { .. } => fold_binary(|a, b| a.wrapping_sub(b), &lhs_c, &rhs_c),
        Instruction::Mul { .. } => fold_binary(|a, b| a.wrapping_mul(b), &lhs_c, &rhs_c),
        Instruction::Eq { .. } => fold_cmp(|a, b| a == b, &lhs_c, &rhs_c),
        Instruction::Lt { .. } => fold_cmp(|a, b| a < b, &lhs_c, &rhs_c),
        Instruction::Gt { .. } => fold_cmp(|a, b| a > b, &lhs_c, &rhs_c),
        _ => return false,
    };

    match folded {
        Some(constant) => {
            *instr = Instruction::Const(constant);
            true
        }
        None => false,
    }
}

fn fold_binary<F: Fn(u32, u32) -> u32>(op: F, lhs: &Constant, rhs: &Constant) -> Option<Constant> {
    match (lhs, rhs) {
        (Constant::I32(a), Constant::I32(b)) => Some(Constant::I32(op(*a, *b))),
        _ => None,
    }
}

fn fold_cmp<F: Fn(u32, u32) -> bool>(op: F, lhs: &Constant, rhs: &Constant) -> Option<Constant> {
    match (lhs, rhs) {
        (Constant::I32(a), Constant::I32(b)) => Some(Constant::I1(op(*a, *b))),
        _ => None,
    }
}
