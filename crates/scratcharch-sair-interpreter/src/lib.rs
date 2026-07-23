use std::collections::HashMap;
use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{GepIndex, Instruction, Terminator};
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::{Constant, ValueId};
use scratcharch_runtime::{ByteMemory, IntrinsicResult, RuntimeError, Value};

enum TermResult {
    Continue,
    Return(Option<RuntimeValue>),
}

#[derive(Debug, Clone)]
pub enum RuntimeValue {
    I1(bool),
    I8(u8),
    I16(u16),
    I32(u32),
    F64(f64),
    Pointer(u32),
}

#[derive(Debug, Clone)]
pub struct Frame {
    func_name: String,
    values: Vec<Option<RuntimeValue>>,
    #[allow(dead_code)]
    return_ty: IrType,
    current_block: String,
    instr_idx: usize,
    prev_block: Option<String>,
}

struct InstrIdMap {
    map: Vec<(String, usize, ValueId)>,
}

impl InstrIdMap {
    fn build(func: &IrFunction) -> Self {
        let mut map = Vec::new();
        let mut cur_id = func.params.len();
        for block in &func.blocks {
            for (idx, instr) in block.instructions.iter().enumerate() {
                if instr.result_type().is_some() {
                    map.push((block.label.clone(), idx, cur_id));
                    cur_id += 1;
                }
            }
        }
        InstrIdMap { map }
    }

    fn lookup(&self, block: &str, idx: usize) -> Option<ValueId> {
        self.map.iter()
            .find(|(b, i, _)| b == block && *i == idx)
            .map(|(_, _, id)| *id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpError {
    UndefinedValue(ValueId),
    TypeMismatch(String),
    DivisionByZero,
    MemoryOutOfBounds(u32),
    NullPointer,
    StackOverflow,
    UndefinedFunction(String),
    UndefinedBlock(String),
    CallStackEmpty,
    CallStackOverflow,
    InvalidGepIndex(String),
    UnsupportedInstruction(String),
    Runtime(RuntimeError),
}

pub struct Interpreter {
    module: IrModule,
    frames: Vec<Frame>,
    memory: Vec<u8>,
    sp: u32,
    stack_limit: u32,
    func_index: HashMap<String, usize>,
    max_frames: usize,
    instr_id_maps: HashMap<String, InstrIdMap>,
}

impl Interpreter {
    pub fn new(module: IrModule, memory_size: u32, stack_limit: u32) -> Self {
        let mut func_index = HashMap::new();
        let mut instr_id_maps = HashMap::new();
        for (i, func) in module.functions.iter().enumerate() {
            func_index.insert(func.name.clone(), i);
            instr_id_maps.insert(func.name.clone(), InstrIdMap::build(func));
        }
        Interpreter {
            module,
            frames: Vec::new(),
            memory: vec![0u8; memory_size as usize],
            sp: memory_size,
            stack_limit,
            func_index,
            max_frames: 1024,
            instr_id_maps,
        }
    }

    pub fn run(&mut self) -> Result<Option<RuntimeValue>, InterpError> {
        let entry_name = self.module.entry.clone();
        let _ = self.func_index.get(&entry_name)
            .ok_or_else(|| InterpError::UndefinedFunction(entry_name.clone()))?;
        self.call_function(&entry_name, Vec::new())
    }

    fn call_function(&mut self, name: &str, args: Vec<RuntimeValue>) -> Result<Option<RuntimeValue>, InterpError> {
        if self.frames.len() >= self.max_frames {
            return Err(InterpError::CallStackOverflow);
        }

        let func_idx = *self.func_index.get(name)
            .ok_or_else(|| InterpError::UndefinedFunction(name.to_string()))?;
        let func = &self.module.functions[func_idx];

        let num_params = func.params.len();
        let mut values: Vec<Option<RuntimeValue>> = vec![None; func.values.len()];
        for (i, arg) in args.into_iter().enumerate() {
            if i < num_params {
                values[i] = Some(arg);
            }
        }

        let entry_block = func.entry_block.clone();
        let frame = Frame {
            func_name: func.name.clone(),
            values,
            return_ty: func.return_ty,
            current_block: entry_block,
            instr_idx: 0,
            prev_block: None,
        };
        self.frames.push(frame);

        let result = self.execute_frame()?;

        Ok(result)
    }

    fn execute_frame(&mut self) -> Result<Option<RuntimeValue>, InterpError> {
        loop {
            let frame_idx = self.frames.len() - 1;
            let block = {
                let frame = &self.frames[frame_idx];
                let func_idx = *self.func_index.get(&frame.func_name).unwrap();
                let func = &self.module.functions[func_idx];
                func.blocks.iter()
                    .find(|b| b.label == frame.current_block)
                    .ok_or_else(|| InterpError::UndefinedBlock(frame.current_block.clone()))?
                    .clone()
            };

            let num_instrs = block.instructions.len();
            let instr_idx = self.frames[frame_idx].instr_idx;

            if instr_idx < num_instrs {
                let instr = &block.instructions[instr_idx];
                self.execute_instruction(instr)?;
                self.frames[frame_idx].instr_idx += 1;
                continue;
            }

            let term_result = self.execute_terminator(&block.terminator)?;
            match term_result {
                TermResult::Continue => {
                    // Block transition handled by execute_terminator; loop back for new block
                    continue;
                }
                TermResult::Return(Some(ret_val)) => {
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(Some(ret_val));
                    }
                    return Ok(Some(ret_val));
                }
                TermResult::Return(None) => {
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(None);
                    }
                    return Ok(None);
                }
            }
        }
    }

    fn execute_instruction(&mut self, instr: &Instruction) -> Result<(), InterpError> {
        match instr {
            Instruction::Add { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        let a = trunc_to_u32(a, *ty);
                        let b = trunc_to_u32(b, *ty);
                        let w = type_width(*ty);
                        let mask = if w >= 32 { 0xFFFFFFFF } else { (1u32 << w) - 1 };
                        RuntimeValue::I32((a.wrapping_add(b)) & mask)
                    }
                    _ => return Err(InterpError::TypeMismatch("add requires integer".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Sub { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        let a = trunc_to_u32(a, *ty);
                        let b = trunc_to_u32(b, *ty);
                        let w = type_width(*ty);
                        let mask = if w >= 32 { 0xFFFFFFFF } else { (1u32 << w) - 1 };
                        RuntimeValue::I32(a.wrapping_sub(b) & mask)
                    }
                    _ => return Err(InterpError::TypeMismatch("sub requires integer".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Mul { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        let a = trunc_to_u32(a, *ty);
                        let b = trunc_to_u32(b, *ty);
                        let w = type_width(*ty);
                        let mask = if w >= 32 { 0xFFFFFFFF } else { (1u32 << w) - 1 };
                        RuntimeValue::I32(a.wrapping_mul(b) & mask)
                    }
                    _ => return Err(InterpError::TypeMismatch("mul requires integer".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Div { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        let a = trunc_to_u32(a, *ty);
                        let b = trunc_to_u32(b, *ty);
                        if b == 0 { return Err(InterpError::DivisionByZero); }
                        RuntimeValue::I32(a / b)
                    }
                    _ => return Err(InterpError::TypeMismatch("div requires integer".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Rem { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        let a = trunc_to_u32(a, *ty);
                        let b = trunc_to_u32(b, *ty);
                        if b == 0 { return Err(InterpError::DivisionByZero); }
                        RuntimeValue::I32(a % b)
                    }
                    _ => return Err(InterpError::TypeMismatch("rem requires integer".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Eq { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        RuntimeValue::I1(trunc_to_u32(a, *ty) == trunc_to_u32(b, *ty))
                    }
                    IrType::F64 => {
                        let a = match a { RuntimeValue::F64(v) => v, _ => return Err(InterpError::TypeMismatch("eq f64".into())) };
                        let b = match b { RuntimeValue::F64(v) => v, _ => return Err(InterpError::TypeMismatch("eq f64".into())) };
                        RuntimeValue::I1(a == b)
                    }
                    _ => return Err(InterpError::TypeMismatch("eq unsupported type".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Lt { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        RuntimeValue::I1(trunc_to_u32(a, *ty) < trunc_to_u32(b, *ty))
                    }
                    _ => return Err(InterpError::TypeMismatch("lt unsupported type".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Gt { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 => {
                        RuntimeValue::I1(trunc_to_u32(a, *ty) > trunc_to_u32(b, *ty))
                    }
                    _ => return Err(InterpError::TypeMismatch("gt unsupported type".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Const(c) => {
                let val = match c {
                    Constant::I1(v) => RuntimeValue::I1(*v),
                    Constant::I8(v) => RuntimeValue::I8(*v),
                    Constant::I16(v) => RuntimeValue::I16(*v),
                    Constant::I32(v) => RuntimeValue::I32(*v),
                    Constant::F64(v) => RuntimeValue::F64(*v),
                };
                self.write_top_value(val);
            }
            Instruction::Alloca { ty, count } => {
                let size = ty.size_in_bytes() * count;
                let new_sp = self.sp.checked_sub(size)
                    .ok_or(InterpError::StackOverflow)?;
                if new_sp < self.stack_limit {
                    return Err(InterpError::StackOverflow);
                }
                self.sp = new_sp;
                self.write_top_value(RuntimeValue::Pointer(self.sp));
            }
            Instruction::Load { ty, addr } => {
                let addr_val = self.read_value(*addr, IrType::Pointer)?;
                let addr = match addr_val {
                    RuntimeValue::Pointer(a) | RuntimeValue::I32(a) => a,
                    _ => return Err(InterpError::TypeMismatch("load expects pointer".into())),
                };
                if addr == 0 {
                    return Err(InterpError::NullPointer);
                }
                let size = ty.size_in_bytes();
                let end = (addr as usize).checked_add(size as usize)
                    .ok_or(InterpError::MemoryOutOfBounds(addr))?;
                if end > self.memory.len() {
                    return Err(InterpError::MemoryOutOfBounds(addr));
                }
                let val = bytes_to_runtime(ty, &self.memory[addr as usize..end]);
                self.write_top_value(val);
            }
            Instruction::Store { ty, value, addr } => {
                let val = self.read_value(*value, *ty)?;
                let addr_val = self.read_value(*addr, IrType::Pointer)?;
                let addr = match addr_val {
                    RuntimeValue::Pointer(a) | RuntimeValue::I32(a) => a,
                    _ => return Err(InterpError::TypeMismatch("store expects pointer".into())),
                };
                if addr == 0 {
                    return Err(InterpError::NullPointer);
                }
                let size = ty.size_in_bytes();
                let end = (addr as usize).checked_add(size as usize)
                    .ok_or(InterpError::MemoryOutOfBounds(addr))?;
                if end > self.memory.len() {
                    return Err(InterpError::MemoryOutOfBounds(addr));
                }
                let bytes = runtime_to_bytes(val, *ty);
                self.memory[addr as usize..end].copy_from_slice(&bytes);
            }
            Instruction::Call { return_ty, callee, args } => {
                let arg_values: Result<Vec<_>, _> = args.iter()
                    .map(|&id| self.read_current_value(id))
                    .collect();
                let arg_values = arg_values?;

                let frame_idx = self.frames.len() - 1;
                let block_label = self.frames[frame_idx].current_block.clone();
                let instr_idx = self.frames[frame_idx].instr_idx;
                let func_name = self.frames[frame_idx].func_name.clone();

                let result = if self.func_index.contains_key(callee) {
                    self.call_function(callee, arg_values)?
                } else {
                    Some(self.dispatch_runtime_intrinsic(callee, return_ty, &arg_values)?)
                };

                if let Some(val) = result {
                    if let Some(map) = self.instr_id_maps.get(&func_name) {
                        if let Some(id) = map.lookup(&block_label, instr_idx) {
                            if id < self.frames[frame_idx].values.len() {
                                self.frames[frame_idx].values[id] = Some(val);
                            } else {
                                self.frames[frame_idx].values.push(Some(val));
                            }
                        }
                    }
                }
            }
            Instruction::Phi { ty: _, incoming } => {
                let frame = self.frames.last().unwrap();
                let pred_label = frame.prev_block.as_ref()
                    .ok_or_else(|| InterpError::UndefinedBlock("no predecessor for phi".into()))?
                    .clone();
                let val = incoming.iter()
                    .find(|(_, l)| l == &pred_label)
                    .map(|(v, _)| *v)
                    .ok_or(InterpError::UndefinedValue(0))?;
                let rv = self.read_current_value(val)?;
                self.write_top_value(rv);
            }
            Instruction::Gep { elem_ty, base, indices, .. } => {
                let base_val = self.read_value(*base, IrType::Pointer)?;
                let mut addr = match base_val {
                    RuntimeValue::Pointer(a) | RuntimeValue::I32(a) => a,
                    _ => return Err(InterpError::TypeMismatch("GEP base must be pointer".into())),
                };
                let current_ty = *elem_ty;
                for index in indices {
                    match index {
                        GepIndex::Dynamic(idx_val) => {
                            let idx = self.read_current_value(*idx_val)?;
                            let idx_u32 = match idx {
                                RuntimeValue::I1(v) => v as u32,
                                RuntimeValue::I8(v) => v as u32,
                                RuntimeValue::I16(v) => v as u32,
                                RuntimeValue::I32(v) => v,
                                _ => return Err(InterpError::TypeMismatch("GEP index must be integer".into())),
                            };
                            addr = addr.wrapping_add(idx_u32 * current_ty.size_in_bytes());
                        }
                        GepIndex::StructField(field) => {
                            addr = addr.wrapping_add(*field * 4);
                        }
                    }
                }
                self.write_top_value(RuntimeValue::Pointer(addr));
            }
        }
        Ok(())
    }

    fn execute_terminator(&mut self, term: &Terminator) -> Result<TermResult, InterpError> {
        let frame_idx = self.frames.len() - 1;
        let cur_block = self.frames[frame_idx].current_block.clone();
        match term {
            Terminator::Branch { target } => {
                self.frames[frame_idx].prev_block = Some(cur_block);
                self.frames[frame_idx].current_block = target.clone();
                self.frames[frame_idx].instr_idx = 0;
                Ok(TermResult::Continue)
            }
            Terminator::CondBranch { condition, true_target, false_target } => {
                let cond = self.read_current_value(*condition)?;
                let take_true = match cond {
                    RuntimeValue::I1(v) => v,
                    _ => return Err(InterpError::TypeMismatch("cond_br condition must be i1".into())),
                };
                let target = if take_true { true_target } else { false_target };
                self.frames[frame_idx].prev_block = Some(cur_block);
                self.frames[frame_idx].current_block = target.clone();
                self.frames[frame_idx].instr_idx = 0;
                Ok(TermResult::Continue)
            }
            Terminator::Return { value } => {
                let val = match value {
                    Some(val_id) => Some(self.read_current_value(*val_id)?),
                    None => None,
                };
                Ok(TermResult::Return(val))
            }
        }
    }

    fn read_current_value(&self, id: ValueId) -> Result<RuntimeValue, InterpError> {
        let frame = self.frames.last().unwrap();
        frame.values.get(id)
            .and_then(|v| v.clone())
            .ok_or(InterpError::UndefinedValue(id))
    }

    fn read_value(&mut self, id: ValueId, _expected_ty: IrType) -> Result<RuntimeValue, InterpError> {
        let frame_idx = self.frames.len() - 1;
        let val = self.frames[frame_idx].values.get(id)
            .and_then(|v| v.clone())
            .ok_or(InterpError::UndefinedValue(id))?;
        Ok(val)
    }

    fn write_top_value(&mut self, val: RuntimeValue) {
        let frame_idx = self.frames.len() - 1;
        let block_label = self.frames[frame_idx].current_block.clone();
        let instr_idx = self.frames[frame_idx].instr_idx;
        let func_name = self.frames[frame_idx].func_name.clone();
        if let Some(map) = self.instr_id_maps.get(&func_name) {
            if let Some(id) = map.lookup(&block_label, instr_idx) {
                if id < self.frames[frame_idx].values.len() {
                    self.frames[frame_idx].values[id] = Some(val);
                } else {
                    self.frames[frame_idx].values.push(Some(val));
                }
            }
        }
    }

    fn dispatch_runtime_intrinsic(
        &mut self,
        name: &str,
        return_ty: &IrType,
        arg_values: &[RuntimeValue],
    ) -> Result<RuntimeValue, InterpError> {
        let args: Vec<u32> = arg_values
            .iter()
            .map(runtime_value_to_u32)
            .collect::<Result<Vec<_>, _>>()?;
        let mut mem = InterpreterMemory(&mut self.memory);
        let result = scratcharch_runtime::dispatch_intrinsic(name, &mut mem, &args)
            .map_err(InterpError::Runtime)?;
        intrinsic_result_to_runtime_value(result, *return_ty)
    }

}

struct InterpreterMemory<'a>(&'a mut Vec<u8>);

impl ByteMemory for InterpreterMemory<'_> {
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError> {
        self.0
            .get(addr as usize)
            .copied()
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })
    }

    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError> {
        let slot = self
            .0
            .get_mut(addr as usize)
            .ok_or(RuntimeError::MemoryOutOfBounds { addr })?;
        *slot = value;
        Ok(())
    }
}

fn runtime_value_to_u32(val: &RuntimeValue) -> Result<u32, InterpError> {
    match val {
        RuntimeValue::I1(v) => Ok(*v as u32),
        RuntimeValue::I8(v) => Ok(*v as u32),
        RuntimeValue::I16(v) => Ok(*v as u32),
        RuntimeValue::I32(v) | RuntimeValue::Pointer(v) => Ok(*v),
        RuntimeValue::F64(_) => Err(InterpError::TypeMismatch(
            "runtime intrinsic argument cannot be f64".into(),
        )),
    }
}

fn intrinsic_result_to_runtime_value(
    res: IntrinsicResult,
    return_ty: IrType,
) -> Result<RuntimeValue, InterpError> {
    match res {
        IntrinsicResult::Void => match return_ty {
            IrType::Void => Ok(RuntimeValue::I32(0)),
            _ => Err(InterpError::TypeMismatch(
                "runtime intrinsic returned void for non-void call".into(),
            )),
        },
        IntrinsicResult::Value(v) => match (v, return_ty) {
            (Value::Pointer(p), _) => Ok(RuntimeValue::Pointer(p)),
            (Value::U32(u), IrType::I32 | IrType::Pointer) => Ok(RuntimeValue::I32(u)),
            (Value::I32(i), IrType::I32) => Ok(RuntimeValue::I32(i as u32)),
            (Value::I32(i), IrType::I1) => Ok(RuntimeValue::I1(i != 0)),
            (Value::U32(u), IrType::I1) => Ok(RuntimeValue::I1(u != 0)),
            _ => Err(InterpError::TypeMismatch(
                "runtime intrinsic return type mismatch".into(),
            )),
        },
    }
}

fn type_width(ty: IrType) -> u32 {
    match ty {
        IrType::I1 => 1,
        IrType::I8 => 8,
        IrType::I16 => 16,
        IrType::I32 => 32,
        _ => 32,
    }
}

fn trunc_to_u32(val: RuntimeValue, _ty: IrType) -> u32 {
    match val {
        RuntimeValue::I1(v) => v as u32,
        RuntimeValue::I8(v) => v as u32,
        RuntimeValue::I16(v) => v as u32,
        RuntimeValue::I32(v) => v,
        _ => 0,
    }
}

fn bytes_to_runtime(ty: &IrType, bytes: &[u8]) -> RuntimeValue {
    match ty {
        IrType::I1 => RuntimeValue::I1(bytes[0] != 0),
        IrType::I8 => RuntimeValue::I8(bytes[0]),
        IrType::I16 => RuntimeValue::I16(u16::from_le_bytes([bytes[0], bytes[1]])),
        IrType::I32 => RuntimeValue::I32(u32::from_le_bytes(bytes.try_into().unwrap())),
        IrType::F64 => RuntimeValue::F64(f64::from_le_bytes(bytes.try_into().unwrap())),
        IrType::Pointer => RuntimeValue::Pointer(u32::from_le_bytes(bytes.try_into().unwrap())),
        IrType::Void => RuntimeValue::I32(0),
    }
}

fn runtime_to_bytes(val: RuntimeValue, _ty: IrType) -> Vec<u8> {
    match val {
        RuntimeValue::I1(v) => vec![v as u8],
        RuntimeValue::I8(v) => vec![v],
        RuntimeValue::I16(v) => v.to_le_bytes().to_vec(),
        RuntimeValue::I32(v) => v.to_le_bytes().to_vec(),
        RuntimeValue::F64(v) => v.to_le_bytes().to_vec(),
        RuntimeValue::Pointer(v) => v.to_le_bytes().to_vec(),
    }
}

impl core::fmt::Display for RuntimeValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RuntimeValue::I1(v) => write!(f, "i1 {}", if *v { 1 } else { 0 }),
            RuntimeValue::I8(v) => write!(f, "i8 {}", v),
            RuntimeValue::I16(v) => write!(f, "i16 {}", v),
            RuntimeValue::I32(v) => write!(f, "i32 {}", v),
            RuntimeValue::F64(v) => write!(f, "f64 {}", v),
            RuntimeValue::Pointer(v) => write!(f, "ptr 0x{v:x}"),
        }
    }
}
