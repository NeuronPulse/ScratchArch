use crate::debug::DebugLoc;
use crate::instruction::{Instruction, Terminator};
use crate::types::IrType;

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
    /// Optional debug locations, parallel to `instructions`.
    ///
    /// `debug_locs[i]` corresponds to `instructions[i]`. The terminator has no
    /// associated debug location in this vector; terminator debug info can be
    /// added later when needed.
    pub debug_locs: Vec<Option<DebugLoc>>,
    pub terminator: Terminator,
}

impl BasicBlock {
    pub fn new(label: impl Into<String>) -> Self {
        BasicBlock {
            label: label.into(),
            instructions: Vec::new(),
            debug_locs: Vec::new(),
            terminator: Terminator::Return { value: None },
        }
    }

    pub fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
        self.debug_locs.push(None);
    }

    pub fn push_with_debug(&mut self, instr: Instruction, debug: Option<DebugLoc>) {
        self.instructions.push(instr);
        self.debug_locs.push(debug);
    }

    pub fn set_terminator(&mut self, term: Terminator) {
        self.terminator = term;
    }

    /// Set the debug location for the instruction at `index`.
    pub fn set_debug_loc(&mut self, index: usize, debug: Option<DebugLoc>) {
        if let Some(slot) = self.debug_locs.get_mut(index) {
            *slot = debug;
        }
    }

    /// Return the debug location for the instruction at `index`, if any.
    pub fn debug_loc(&self, index: usize) -> Option<&DebugLoc> {
        self.debug_locs.get(index).and_then(|d| d.as_ref())
    }

    pub fn return_type(&self) -> Option<IrType> {
        self.instructions.last().and_then(|i| i.result_type())
    }

    pub fn referenced_labels(&self) -> Vec<String> {
        self.terminator.referenced_labels()
    }
}
