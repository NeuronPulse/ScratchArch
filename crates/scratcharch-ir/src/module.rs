use std::collections::HashMap;
use crate::function::IrFunction;

#[derive(Debug, Clone)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub entry: String,
    func_index: HashMap<String, usize>,
}

impl IrModule {
    pub fn new(entry: impl Into<String>) -> Self {
        IrModule {
            functions: Vec::new(),
            entry: entry.into(),
            func_index: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, func: IrFunction) {
        self.func_index.insert(func.name.clone(), self.functions.len());
        self.functions.push(func);
    }

    pub fn get_function(&self, name: &str) -> Option<&IrFunction> {
        self.func_index.get(name).map(|&i| &self.functions[i])
    }

    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IrFunction> {
        let idx = *self.func_index.get(name)?;
        self.functions.get_mut(idx)
    }

    pub fn function_index(&self, name: &str) -> Option<usize> {
        self.func_index.get(name).copied()
    }

    pub fn validate(&self) -> Result<(), String> {
        let entry_idx = self.function_index(&self.entry)
            .ok_or_else(|| format!("entry function '{}' not found", self.entry))?;
        for func in &self.functions {
            func.validate()?;
        }
        let _ = entry_idx;
        Ok(())
    }
}
