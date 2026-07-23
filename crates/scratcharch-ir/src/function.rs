use std::collections::{HashMap, HashSet};
use crate::block::BasicBlock;
use crate::instruction::Instruction;
use crate::types::IrType;
use crate::value::{SsaValue, ValueId};

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub return_ty: IrType,
    pub params: Vec<(IrType, String)>,
    pub blocks: Vec<BasicBlock>,
    pub values: Vec<SsaValue>,
    pub entry_block: String,
    value_counter: ValueId,
}

impl IrFunction {
    pub fn new(name: impl Into<String>, return_ty: IrType) -> Self {
        IrFunction {
            name: name.into(),
            return_ty,
            params: Vec::new(),
            blocks: Vec::new(),
            values: Vec::new(),
            entry_block: "entry".to_string(),
            value_counter: 0,
        }
    }

    pub fn add_param(&mut self, ty: IrType, name: impl Into<String>) -> ValueId {
        let name_str: String = name.into();
        let id = self.value_counter;
        self.value_counter += 1;
        self.values.push(SsaValue::new(id, ty, Some(name_str.clone())));
        self.params.push((ty, name_str));
        id
    }

    pub fn new_value(&mut self, ty: IrType, name: Option<impl Into<String>>) -> ValueId {
        let id = self.value_counter;
        self.value_counter += 1;
        self.values.push(SsaValue::new(id, ty, name.map(|n| n.into())));
        id
    }

    pub fn add_block(&mut self, block: BasicBlock) {
        self.blocks.push(block);
    }

    pub fn get_value(&self, id: ValueId) -> Option<&SsaValue> {
        self.values.get(id)
    }

    pub fn block_index(&self, label: &str) -> Option<usize> {
        self.blocks.iter().position(|b| b.label == label)
    }

    pub fn value_counter(&self) -> ValueId {
        self.value_counter
    }

    pub fn set_value_counter(&mut self, counter: ValueId) {
        self.value_counter = counter;
    }

    pub(crate) fn build_pred_map(&self) -> HashMap<String, Vec<String>> {
        let mut preds: HashMap<String, Vec<String>> = HashMap::new();
        for block in &self.blocks {
            preds.entry(block.label.clone()).or_default();
            for target in block.referenced_labels() {
                preds.entry(target).or_default().push(block.label.clone());
            }
        }
        preds
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() {
            return Err(format!("function '{}' has no blocks", self.name));
        }
        let entry_idx = self.block_index(&self.entry_block)
            .ok_or_else(|| format!("entry block '{}' not found", self.entry_block))?;
        if entry_idx != 0 {
            return Err("entry block must be first".to_string());
        }

        let pred_map = self.build_pred_map();

        // Check that all referenced labels exist
        for block in &self.blocks {
            for label in block.referenced_labels() {
                if self.block_index(&label).is_none() {
                    return Err(format!("block '{}' references undefined block '{}'",
                        block.label, label));
                }
            }
        }

        // Check phi incoming labels reference valid predecessor blocks
        for block in &self.blocks {
            let expected_preds: HashSet<&str> = pred_map.get(&block.label)
                .map(|p| p.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();
            for instr in &block.instructions {
                if let Instruction::Phi { incoming, .. } = instr {
                    for (_, pred_label) in incoming {
                        if !expected_preds.contains(pred_label.as_str()) {
                            return Err(format!(
                                "phi in block '{}' references non-predecessor block '{}'",
                                block.label, pred_label));
                        }
                    }
                }
            }
        }

        // Check reachability: all blocks must be reachable from entry
        let mut reachable: HashSet<String> = HashSet::new();
        let mut worklist: Vec<String> = vec![self.entry_block.clone()];
        while let Some(label) = worklist.pop() {
            if !reachable.insert(label.clone()) {
                continue;
            }
            if let Some(block) = self.blocks.iter().find(|b| b.label == label) {
                for target in block.referenced_labels() {
                    worklist.push(target);
                }
            }
        }
        for block in &self.blocks {
            if !reachable.contains(&block.label) {
                return Err(format!(
                    "block '{}' is not reachable from entry", block.label));
            }
        }

        Ok(())
    }
}
