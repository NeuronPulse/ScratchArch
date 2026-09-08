use std::collections::{HashMap, HashSet};
use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{GepIndex, Instruction, Terminator};
use scratcharch_ir::value::{Constant, ValueId};

pub struct FunctionAnalysis {
    def_map: HashMap<ValueId, (String, usize)>,
    use_map: HashMap<ValueId, Vec<(String, usize)>>,
    pred_map: HashMap<String, Vec<String>>,
    succ_map: HashMap<String, Vec<String>>,
    reachable: HashSet<String>,
    constant_map: HashMap<ValueId, Constant>,
}

impl FunctionAnalysis {
    pub fn build(func: &IrFunction) -> Self {
        let def_map = build_def_map(func);
        let use_map = build_use_map(func);
        let (pred_map, succ_map) = build_cfg(func);
        let reachable = compute_reachable(func, &succ_map);
        let constant_map = build_constant_map(func);

        FunctionAnalysis {
            def_map,
            use_map,
            pred_map,
            succ_map,
            reachable,
            constant_map,
        }
    }

    pub fn is_constant_value(&self, id: ValueId) -> bool {
        self.constant_map.contains_key(&id)
    }

    pub fn get_constant(&self, id: ValueId) -> Option<&Constant> {
        self.constant_map.get(&id)
    }

    pub fn has_users(&self, id: ValueId) -> bool {
        self.use_map.get(&id).is_some_and(|u| !u.is_empty())
    }

    pub fn num_users(&self, id: ValueId) -> usize {
        self.use_map.get(&id).map_or(0, |u| u.len())
    }

    pub fn is_block_reachable(&self, label: &str) -> bool {
        self.reachable.contains(label)
    }

    pub fn predecessors(&self, label: &str) -> Option<&Vec<String>> {
        self.pred_map.get(label)
    }

    pub fn successors(&self, label: &str) -> Option<&Vec<String>> {
        self.succ_map.get(label)
    }

    pub fn reachable_blocks(&self) -> &HashSet<String> {
        &self.reachable
    }

    pub fn def_map(&self) -> &HashMap<ValueId, (String, usize)> {
        &self.def_map
    }

    pub fn use_map(&self) -> &HashMap<ValueId, Vec<(String, usize)>> {
        &self.use_map
    }

    pub fn pred_map(&self) -> &HashMap<String, Vec<String>> {
        &self.pred_map
    }
}

fn build_def_map(func: &IrFunction) -> HashMap<ValueId, (String, usize)> {
    let mut map = HashMap::new();
    let mut cur_id = func.params.len();
    for block in &func.blocks {
        for (idx, instr) in block.instructions.iter().enumerate() {
            if instr.result_type().is_some() {
                map.insert(cur_id, (block.label.clone(), idx));
                cur_id += 1;
            }
        }
    }
    map
}

fn build_constant_map(func: &IrFunction) -> HashMap<ValueId, Constant> {
    let mut map = HashMap::new();
    let mut cur_id = func.params.len();
    for block in &func.blocks {
        for instr in &block.instructions {
            if let Instruction::Const(c) = instr {
                map.insert(cur_id, c.clone());
            }
            if instr.result_type().is_some() {
                cur_id += 1;
            }
        }
    }
    map
}

fn build_use_map(func: &IrFunction) -> HashMap<ValueId, Vec<(String, usize)>> {
    let mut uses: HashMap<ValueId, Vec<(String, usize)>> = HashMap::new();
    for block in &func.blocks {
        for (idx, instr) in block.instructions.iter().enumerate() {
            let deps = instruction_operands(instr);
            for &dep in &deps {
                uses.entry(dep).or_default().push((block.label.clone(), idx));
            }
        }
        let term_deps = terminator_operands(&block.terminator);
        for &dep in &term_deps {
            uses.entry(dep).or_default().push((block.label.clone(), usize::MAX));
        }
    }
    uses
}

fn instruction_operands(instr: &Instruction) -> Vec<ValueId> {
    match instr {
        Instruction::Add { lhs, rhs, .. }
        | Instruction::Sub { lhs, rhs, .. }
        | Instruction::Mul { lhs, rhs, .. }
        | Instruction::Div { lhs, rhs, .. }
        | Instruction::Rem { lhs, rhs, .. }
        | Instruction::Eq { lhs, rhs, .. }
        | Instruction::Lt { lhs, rhs, .. }
        | Instruction::Gt { lhs, rhs, .. } => vec![*lhs, *rhs],
        Instruction::Const(_) => vec![],
        Instruction::Alloca { .. } => vec![],
        Instruction::Load { addr, .. } => vec![*addr],
        Instruction::Store { value, addr, .. } => vec![*value, *addr],
        Instruction::Call { args, .. } => args.clone(),
        Instruction::Phi { incoming, .. } => incoming.iter().map(|(v, _)| *v).collect(),
        Instruction::Cast { value, .. } => vec![*value],
        Instruction::Select { condition, then_value, else_value, .. } => {
            vec![*condition, *then_value, *else_value]
        }
        Instruction::Gep { base, indices, .. } => {
            let mut deps = vec![*base];
            for index in indices {
                if let GepIndex::Dynamic(id) = index {
                    deps.push(*id);
                }
            }
            deps
        }
    }
}

fn terminator_operands(term: &Terminator) -> Vec<ValueId> {
    match term {
        Terminator::Branch { .. } => vec![],
        Terminator::CondBranch { condition, .. } => vec![*condition],
        Terminator::Return { value } => value.map_or(vec![], |v| vec![v]),
        Terminator::Unreachable => vec![],
    }
}

fn build_cfg(func: &IrFunction) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut preds: HashMap<String, Vec<String>> = HashMap::new();
    let mut succs: HashMap<String, Vec<String>> = HashMap::new();

    for block in &func.blocks {
        let label = block.label.clone();
        succs.entry(label.clone()).or_default();
        preds.entry(label.clone()).or_default();

        let targets = block.referenced_labels();
        for target in targets {
            succs.entry(label.clone()).or_default().push(target.clone());
            preds.entry(target).or_default().push(label.clone());
        }
    }

    (preds, succs)
}

fn compute_reachable(
    func: &IrFunction,
    succ_map: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut worklist = vec![func.entry_block.clone()];
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        if let Some(succs) = succ_map.get(&label) {
            for succ in succs {
                worklist.push(succ.clone());
            }
        }
    }
    reachable
}
