use std::collections::HashMap;
use crate::instruction::{Instruction, Label, FunctionName};

#[derive(Debug, Clone)]
pub struct Function {
    pub name: FunctionName,
    pub instructions: Vec<(Option<Label>, Instruction)>,
    label_map: HashMap<Label, usize>,
    /// Number of argument cells the callee pops from the operand stack on entry.
    pub param_cells: u32,
    /// Total number of local slots allocated for this function (includes parameters).
    pub local_count: u32,
    /// Number of cells this function returns (0 for void).
    pub return_cells: u32,
}

impl Function {
    pub fn new(name: impl Into<String>) -> Self {
        Function {
            name: name.into(),
            instructions: Vec::new(),
            label_map: HashMap::new(),
            param_cells: 0,
            local_count: 0,
            return_cells: 1,
        }
    }

    pub fn push(&mut self, label: Option<impl Into<Label>>, instr: Instruction) {
        if let Some(l) = label {
            let label_str: Label = l.into();
            self.label_map
                .insert(label_str, self.instructions.len());
        }
        self.instructions.push((None, instr));
    }

    pub fn get_label_index(&self, label: &str) -> Option<usize> {
        self.label_map.get(label).copied()
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<Function>,
    pub entry: FunctionName,
    func_index: HashMap<FunctionName, usize>,
}

impl Program {
    pub fn new(entry: impl Into<FunctionName>) -> Self {
        Program {
            functions: Vec::new(),
            entry: entry.into(),
            func_index: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, func: Function) {
        self.func_index
            .insert(func.name.clone(), self.functions.len());
        self.functions.push(func);
    }

    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.func_index.get(name).map(|&i| &self.functions[i])
    }

    pub fn get_function_index(&self, name: &str) -> Option<usize> {
        self.func_index.get(name).copied()
    }
}
