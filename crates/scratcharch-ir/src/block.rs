use crate::instruction::{Instruction, Terminator};
use crate::types::IrType;

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

impl BasicBlock {
    pub fn new(label: impl Into<String>) -> Self {
        BasicBlock {
            label: label.into(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }
    }

    pub fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
    }

    pub fn set_terminator(&mut self, term: Terminator) {
        self.terminator = term;
    }

    pub fn return_type(&self) -> Option<IrType> {
        self.instructions.last().and_then(|i| i.result_type())
    }

    pub fn referenced_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        match &self.terminator {
            Terminator::Branch { target } => labels.push(target.clone()),
            Terminator::CondBranch { true_target, false_target, .. } => {
                labels.push(true_target.clone());
                labels.push(false_target.clone());
            }
            Terminator::Return { .. } => {}
        }
        labels
    }
}
