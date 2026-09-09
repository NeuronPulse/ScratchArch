use std::collections::HashMap;
use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{CastOp, GepIndex, Instruction, Terminator};
use scratcharch_ir::r#module::{IrModule, STATIC_DATA_BASE};
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
    I64(u64),
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
    /// Control reached an LLVM `unreachable` terminator. This is a trap: the IR
    /// declares the path impossible, so the program is erroneous.
    Trap,
    /// The module's static data segment does not fit below the stack floor. The
    /// segment is seeded at [`STATIC_DATA_BASE`]; if it reached into the stack's
    /// region the stack would silently overwrite global data, so this is an
    /// explicit error instead. Raise `stack_limit` (or shrink the globals).
    StaticDataTooLarge {
        base: u32,
        size: usize,
        stack_limit: u32,
    },
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
    static_seeded: bool,
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
            static_seeded: false,
        }
    }

    /// Copy the module's static data segment (LLVM globals) into the low memory
    /// region below the stack floor. Runs once, before the first `run()`. The
    /// segment must fit entirely below `stack_limit`, otherwise the stack could
    /// grow down over it later — that is reported as an error, never silently
    /// allowed.
    fn seed_static_data(&mut self) -> Result<(), InterpError> {
        if self.static_seeded {
            return Ok(());
        }
        self.static_seeded = true;
        let data = &self.module.static_data;
        if data.image.is_empty() {
            return Ok(());
        }
        let size = data.image.len();
        let end = STATIC_DATA_BASE as usize + size;
        if end > self.stack_limit as usize || end > self.memory.len() {
            return Err(InterpError::StaticDataTooLarge {
                base: STATIC_DATA_BASE,
                size,
                stack_limit: self.stack_limit,
            });
        }
        self.memory[STATIC_DATA_BASE as usize..end].copy_from_slice(&data.image);
        Ok(())
    }

    /// Set the maximum number of call frames allowed before execution fails with
    /// [`InterpError::CallStackOverflow`].
    pub fn set_max_frames(&mut self, max_frames: usize) {
        self.max_frames = max_frames;
    }

    pub fn run(&mut self) -> Result<Option<RuntimeValue>, InterpError> {
        self.seed_static_data()?;
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
                let result = int_binop(BinOp::Add, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Sub { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_binop(BinOp::Sub, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Mul { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_binop(BinOp::Mul, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Div { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_binop(BinOp::Div, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Rem { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_binop(BinOp::Rem, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::And { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::And, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Or { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::Or, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Xor { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::Xor, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Shl { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::Shl, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Lshr { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::Lshr, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Ashr { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = int_bitop(BitOp::Ashr, *ty, &a, &b)?;
                self.write_top_value(result);
            }
            Instruction::Eq { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64 => {
                        RuntimeValue::I1(int_bits(&a, *ty) == int_bits(&b, *ty))
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
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64 => {
                        RuntimeValue::I1(int_bits(&a, *ty) < int_bits(&b, *ty))
                    }
                    _ => return Err(InterpError::TypeMismatch("lt unsupported type".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Gt { ty, lhs, rhs } => {
                let a = self.read_value(*lhs, *ty)?;
                let b = self.read_value(*rhs, *ty)?;
                let result = match ty {
                    IrType::I1 | IrType::I8 | IrType::I16 | IrType::I32 | IrType::I64 => {
                        RuntimeValue::I1(int_bits(&a, *ty) > int_bits(&b, *ty))
                    }
                    _ => return Err(InterpError::TypeMismatch("gt unsupported type".into())),
                };
                self.write_top_value(result);
            }
            Instruction::Select { condition, then_value, else_value, .. } => {
                let cond = self.read_current_value(*condition)?;
                let take_true = match cond {
                    RuntimeValue::I1(v) => v,
                    _ => {
                        return Err(InterpError::TypeMismatch(
                            "select condition must be i1".into(),
                        ))
                    }
                };
                // LLVM requires both operands to dominate the select, so both
                // must already be materialized regardless of the chosen branch.
                let then_v = self.read_current_value(*then_value)?;
                let else_v = self.read_current_value(*else_value)?;
                let result = if take_true { then_v } else { else_v };
                self.write_top_value(result);
            }
            Instruction::Const(c) => {
                let val = match c {
                    Constant::I1(v) => RuntimeValue::I1(*v),
                    Constant::I8(v) => RuntimeValue::I8(*v),
                    Constant::I16(v) => RuntimeValue::I16(*v),
                    Constant::I32(v) => RuntimeValue::I32(*v),
                    Constant::I64(v) => RuntimeValue::I64(*v),
                    Constant::F64(v) => RuntimeValue::F64(*v),
                };
                self.write_top_value(val);
            }
            Instruction::Cast { op, from_ty, to_ty, value } => {
                let v = self.read_value(*value, *from_ty)?;
                let result = cast_value(*op, *from_ty, *to_ty, &v)?;
                self.write_top_value(result);
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
                let bytes = store_bytes(&val, *ty)?;
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
                                // GEP indices on 64-bit hosts are i64; SA48
                                // addresses are 32-bit, so the low word is the
                                // offset (mod 2^32).
                                RuntimeValue::I64(v) => v as u32,
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
            Terminator::Unreachable => Err(InterpError::Trap),
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
        // LLVM bit intrinsics (`llvm.bswap/ctpop/ctlz/cttz.iN`) are resolved
        // directly from the SSA values: they are pure bitwise reference
        // expansions over the declared width and need no memory access. This is
        // the interpreter-level "reference implementation" — no new SAIR or ISA
        // instruction is involved.
        // Memory intrinsics (`llvm.memcpy`/`llvm.memmove`/`llvm.memset`) write
        // through the flat SAIR memory and are resolved first. Unknown `llvm.*`
        // names fall through to the pure bit intrinsics below, which reject
        // them explicitly.
        if name.starts_with("llvm.") {
            if let Some(result) = self.dispatch_memory_intrinsic(name, arg_values)? {
                return Ok(result);
            }
            return self
                .dispatch_llvm_intrinsic(name, arg_values)?
                .ok_or_else(|| InterpError::UnsupportedInstruction(format!(
                    "unsupported llvm intrinsic: {name}"
                )));
        }
        let args: Vec<u32> = arg_values
            .iter()
            .map(runtime_value_to_u32)
            .collect::<Result<Vec<_>, _>>()?;
        let mut mem = InterpreterMemory(&mut self.memory);
        let result = scratcharch_runtime::dispatch_intrinsic(name, &mut mem, &args)
            .map_err(InterpError::Runtime)?;
        intrinsic_result_to_runtime_value(result, *return_ty)
    }

    /// Resolve a `llvm.bswap/ctpop/ctlz/cttz.iN` call, where `N` is carried in
    /// the intrinsic name (`llvm.ctpop.i32` → 32-bit). `ctlz`/`cttz` take an
    /// `i1` "is_zero_undef" immarg that clang always emits as `false`; the
    /// argument is accepted and ignored (SAIR has no poison). Returns `None`
    /// for names that are not one of these four families so the caller can
    /// produce an explicit "unsupported intrinsic" diagnostic.
    fn dispatch_llvm_intrinsic(
        &self,
        name: &str,
        arg_values: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, InterpError> {
        let rest = name.strip_prefix("llvm.").unwrap();
        // `rest` = `<family>.i<width>`. Split on the trailing `.iN` so the
        // family may itself contain dots (e.g. `llvm.fshl.i32` in the future).
        let dot = rest.rfind(".i").ok_or_else(|| {
            InterpError::UnsupportedInstruction(format!("malformed llvm intrinsic: {name}"))
        })?;
        let family = &rest[..dot];
        let width: u32 = rest[dot + 2..].parse().map_err(|_| {
            InterpError::UnsupportedInstruction(format!("malformed llvm intrinsic: {name}"))
        })?;
        if width != 8 && width != 16 && width != 32 && width != 64 {
            return Err(InterpError::UnsupportedInstruction(format!(
                "unsupported llvm intrinsic width {width} in {name}"
            )));
        }
        // The first argument is the value; ctlz/cttz carry a second i1 arg.
        let value = arg_values.first().ok_or_else(|| {
            InterpError::TypeMismatch(format!("{name} requires at least one argument"))
        })?;
        let bits = value_bits(value)?;
        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
        let x = bits & mask;

        let result = match family {
            "bswap" => bswap(x, u64::from(width / 8)),
            "ctpop" => x.count_ones() as u64,
            "ctlz" => {
                if x == 0 {
                    width as u64
                } else {
                    // `x` is masked to `width` bits, so its u64 leading zeros
                    // include the (64 - width) padding bits above the width.
                    (x.leading_zeros().saturating_sub(64 - width)) as u64
                }
            }
            "cttz" => {
                if x == 0 {
                    width as u64
                } else {
                    x.trailing_zeros() as u64
                }
            }
            _ => return Ok(None),
        };
        // Sub-64 results are carried as masked I32 (matching SAIR's carrier);
        // 64-bit results use I64. The caller reads these back and masks by the
        // instruction's own type, so widths stay exact.
        let result = if width == 64 {
            RuntimeValue::I64(result)
        } else {
            RuntimeValue::I32(result as u32)
        };
        Ok(Some(result))
    }

    /// Resolve `llvm.memcpy.*`/`llvm.memmove.*`/`llvm.memset.*` calls against the
    /// interpreter's flat byte memory, using the same address/bounds rules as
    /// SAIR `Load`/`Store` (little-endian, byte-addressed, `NullPointer` for a
    /// zero destination). `memmove` and `memcpy` copy through a temporary so
    /// overlapping regions stay well-defined (`memmove` semantics); `memcpy`
    /// semantics coincide when the regions do not overlap, which is the only
    /// case where LLVM defines `memcpy` anyway.
    ///
    /// The length argument is a byte count regardless of its integer width
    /// (clang emits `i64` for `p0.p0.i64`, `i32` for older forms). The trailing
    /// `isvolatile` immarg is accepted and ignored: SAIR memory has no volatile
    /// model, matching the existing load/store treatment.
    fn dispatch_memory_intrinsic(
        &mut self,
        name: &str,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, InterpError> {
        enum MemOp {
            Copy,
            Move,
            Set,
        }
        // The family is `llvm.memcpy`/`llvm.memmove`/`llvm.memset` and the
        // remainder is its *variant* (`p<d>.p<s>.i<lenwidth>` for the two-address
        // intrinsics, `p<d>.i<lenwidth>` for memset). The variant must be
        // validated structurally so that a *different* intrinsic that merely
        // shares the family prefix — e.g. `llvm.memcpy.inline.*` or
        // `llvm.memcpy.element.unordered.*` — is rejected with an explicit
        // diagnostic instead of silently executed. Address spaces and the
        // length width do not change the byte-copy semantics, so any `p<N>`
        // forms are accepted.
        let op = if let Some(rest) = name.strip_prefix("llvm.memcpy.") {
            let segs: Vec<&str> = rest.split('.').collect();
            if segs.len() == 3 && segs[0].starts_with('p') && segs[1].starts_with('p') {
                Some(MemOp::Copy)
            } else {
                return Err(InterpError::UnsupportedInstruction(format!(
                    "unsupported llvm.memcpy variant: {name}"
                )));
            }
        } else if let Some(rest) = name.strip_prefix("llvm.memmove.") {
            let segs: Vec<&str> = rest.split('.').collect();
            if segs.len() == 3 && segs[0].starts_with('p') && segs[1].starts_with('p') {
                Some(MemOp::Move)
            } else {
                return Err(InterpError::UnsupportedInstruction(format!(
                    "unsupported llvm.memmove variant: {name}"
                )));
            }
        } else if let Some(rest) = name.strip_prefix("llvm.memset.") {
            let segs: Vec<&str> = rest.split('.').collect();
            if segs.len() == 2 && segs[0].starts_with('p') {
                Some(MemOp::Set)
            } else {
                return Err(InterpError::UnsupportedInstruction(format!(
                    "unsupported llvm.memset variant: {name}"
                )));
            }
        } else {
            None
        };
        let op = match op {
            Some(op) => op,
            None => return Ok(None),
        };
        if args.len() < 3 {
            return Err(InterpError::TypeMismatch(format!(
                "{name} requires (dst, src/val, len) at minimum, got {} arguments",
                args.len()
            )));
        }

        let n = as_mem_len(&args[2])? as usize;
        match op {
            MemOp::Copy | MemOp::Move => {
                let dst = as_mem_addr(&args[0])?;
                let src = as_mem_addr(&args[1])?;
                if n == 0 {
                    return Ok(Some(RuntimeValue::I32(0)));
                }
                if dst == 0 {
                    return Err(InterpError::NullPointer);
                }
                let d_end = (dst as usize)
                    .checked_add(n)
                    .ok_or(InterpError::MemoryOutOfBounds(dst))?;
                let s_end = (src as usize)
                    .checked_add(n)
                    .ok_or(InterpError::MemoryOutOfBounds(src))?;
                if d_end > self.memory.len() || s_end > self.memory.len() {
                    return Err(InterpError::MemoryOutOfBounds(dst));
                }
                let bytes = self.memory[src as usize..s_end].to_vec();
                self.memory[dst as usize..d_end].copy_from_slice(&bytes);
            }
            MemOp::Set => {
                let dst = as_mem_addr(&args[0])?;
                let byte = as_mem_byte(&args[1])?;
                if n == 0 {
                    return Ok(Some(RuntimeValue::I32(0)));
                }
                if dst == 0 {
                    return Err(InterpError::NullPointer);
                }
                let d_end = (dst as usize)
                    .checked_add(n)
                    .ok_or(InterpError::MemoryOutOfBounds(dst))?;
                if d_end > self.memory.len() {
                    return Err(InterpError::MemoryOutOfBounds(dst));
                }
                self.memory[dst as usize..d_end].fill(byte);
            }
        }
        Ok(Some(RuntimeValue::I32(0)))
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
        // Runtime intrinsics take u32 words. An i64 argument is truncated to
        // its low 32 bits, consistent with the 32-bit addressing model. Intrinsics
        // whose semantics actually need the high word must reject this at
        // registry level; no such intrinsic is currently registered.
        RuntimeValue::I64(v) => Ok(*v as u32),
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

/// Mask `bits` down to the bit width of `ty` (used for sub-32-bit values that
/// are carried in a 32-bit container). For widths >= 64 the value is unmasked.
fn mask_to_width(bits: u64, ty: IrType) -> u64 {
    match ty.integer_width() {
        Some(w) if w < 64 => bits & ((1u64 << w) - 1),
        _ => bits,
    }
}

/// Reinterpret a runtime value as an unsigned `ty`-bit integer. Values whose
/// SSA type is i8/i16 are already masked, so this is idempotent.
fn int_bits(val: &RuntimeValue, ty: IrType) -> u64 {
    let raw = match val {
        RuntimeValue::I1(v) => *v as u64,
        RuntimeValue::I8(v) => *v as u64,
        RuntimeValue::I16(v) => *v as u64,
        RuntimeValue::I32(v) => *v as u64,
        RuntimeValue::I64(v) => *v,
        _ => 0,
    };
    mask_to_width(raw, ty)
}

/// Raw unsigned bits carried by an integer runtime value. Sub-64-bit values are
/// already masked in their container at write time, so — unlike [`int_bits`],
/// which also masks against a requested type — no type argument is needed.
fn value_bits(val: &RuntimeValue) -> Result<u64, InterpError> {
    match val {
        RuntimeValue::I1(v) => Ok(*v as u64),
        RuntimeValue::I8(v) => Ok(*v as u64),
        RuntimeValue::I16(v) => Ok(*v as u64),
        RuntimeValue::I32(v) | RuntimeValue::Pointer(v) => Ok(*v as u64),
        RuntimeValue::I64(v) => Ok(*v),
        RuntimeValue::F64(_) => Err(InterpError::TypeMismatch(
            "integer intrinsic argument cannot be f64".into(),
        )),
    }
}

/// Interpret a runtime value as a byte address (pointer or address integer).
fn as_mem_addr(val: &RuntimeValue) -> Result<u32, InterpError> {
    match val {
        RuntimeValue::Pointer(a) | RuntimeValue::I32(a) => Ok(*a),
        _ => Err(InterpError::TypeMismatch(
            "memory intrinsic address must be a pointer or address integer".into(),
        )),
    }
}

/// Interpret a runtime value as a byte count for a memory intrinsic.
fn as_mem_len(val: &RuntimeValue) -> Result<u64, InterpError> {
    match val {
        RuntimeValue::I1(v) => Ok(*v as u64),
        RuntimeValue::I8(v) => Ok(*v as u64),
        RuntimeValue::I16(v) => Ok(*v as u64),
        RuntimeValue::I32(v) | RuntimeValue::Pointer(v) => Ok(*v as u64),
        RuntimeValue::I64(v) => Ok(*v),
        RuntimeValue::F64(_) => Err(InterpError::TypeMismatch(
            "memory intrinsic length cannot be f64".into(),
        )),
    }
}

/// Interpret a runtime value as a byte for `llvm.memset`.
fn as_mem_byte(val: &RuntimeValue) -> Result<u8, InterpError> {
    match val {
        RuntimeValue::I1(v) => Ok(*v as u8),
        RuntimeValue::I8(v) => Ok(*v),
        RuntimeValue::I16(v) => Ok(*v as u8),
        RuntimeValue::I32(v) | RuntimeValue::Pointer(v) => Ok(*v as u8),
        RuntimeValue::I64(v) => Ok(*v as u8),
        RuntimeValue::F64(_) => Err(InterpError::TypeMismatch(
            "memory intrinsic value cannot be f64".into(),
        )),
    }
}

/// Byte-reverse the low `nbytes` bytes of `x` (LLVM `llvm.bswap.i<N>`); upper
/// bits are left untouched and masked by the caller.
fn bswap(x: u64, nbytes: u64) -> u64 {
    let mut out = 0u64;
    for i in 0..nbytes {
        let byte = (x >> (i * 8)) & 0xff;
        out |= byte << ((nbytes - 1 - i) * 8);
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}/// Apply a wrapping binary integer op on two values of the given type.
///
/// SAIR arithmetic is wrapping two's-complement. i8/i16/i32 results are carried
/// as masked `RuntimeValue::I32` (matching the pre-existing behaviour); i64
/// results are carried as `RuntimeValue::I64`.
fn int_binop(op: BinOp, ty: IrType, a: &RuntimeValue, b: &RuntimeValue) -> Result<RuntimeValue, InterpError> {
    let w = ty
        .integer_width()
        .ok_or_else(|| InterpError::TypeMismatch(format!("{op:?} requires integer type, got {ty}")))?;
    let a = int_bits(a, ty);
    let b = int_bits(b, ty);
    let (res, overflowed) = match op {
        BinOp::Add => (a.wrapping_add(b), false),
        BinOp::Sub => (a.wrapping_sub(b), false),
        BinOp::Mul => (a.wrapping_mul(b), false),
        BinOp::Div => {
            if b == 0 {
                return Err(InterpError::DivisionByZero);
            }
            // LLVM `udiv` (unsigned); `sdiv` is translated by the SAIR frontends
            // into the trunc-modelled form available today. See LLVM_COMPATIBILITY.
            (a / b, false)
        }
        BinOp::Rem => {
            if b == 0 {
                return Err(InterpError::DivisionByZero);
            }
            (a % b, false)
        }
    };
    let _ = overflowed;
    if w >= 64 {
        Ok(RuntimeValue::I64(res))
    } else {
        Ok(RuntimeValue::I32(res as u32))
    }
}

#[derive(Debug, Clone, Copy)]
enum BitOp {
    And,
    Or,
    Xor,
    Shl,
    Lshr,
    Ashr,
}

/// Apply a wrapping bitwise op on two values of the given type.
///
/// SAIR bitwise/shift semantics follow the arithmetic convention: results are
/// two's-complement wrapping and the carrier for a sub-64-bit integer is an
/// I32 whose low `w` bits hold the value. Shift amounts live in a poison region
/// above the width; this stack defines them deterministically (so the VM
/// backend can match it exactly) as `amount mod width`, i.e. the effective
/// amount is `amount & (width - 1)`. Within the defined region (amount < width)
/// the result is the exact width-bit shift. `Ashr` replicates the sign bit of
/// the two's-complement `width`-bit pattern.
fn int_bitop(op: BitOp, ty: IrType, a: &RuntimeValue, b: &RuntimeValue) -> Result<RuntimeValue, InterpError> {
    let w = ty
        .integer_width()
        .ok_or_else(|| InterpError::TypeMismatch(format!("{op:?} requires integer type, got {ty}")))?;
    let a = int_bits(a, ty);
    let b = int_bits(b, ty);
    let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
    let res = match op {
        BitOp::And => a & b,
        BitOp::Or => a | b,
        BitOp::Xor => a ^ b,
        BitOp::Shl | BitOp::Lshr | BitOp::Ashr => {
            // Effective shift amount = amount mod width (see doc comment).
            let eff = if w >= 64 { (b & 63) as u32 } else { (b & u64::from(w - 1)) as u32 };
            if eff == 0 {
                a & mask
            } else {
                let shifted = match op {
                    BitOp::Shl => a << eff,
                    BitOp::Lshr => a >> eff,
                    _ => {
                        // Arithmetic right shift: sign-fill the top `eff` bits
                        // of the width-bit pattern.
                        let sign = (a & (1u64 << (w - 1))) != 0;
                        if sign {
                            let fill = mask ^ ((1u64 << (w - eff)) - 1);
                            (a >> eff) | fill
                        } else {
                            a >> eff
                        }
                    }
                };
                shifted & mask
            }
        }
    };
    if w >= 64 {
        Ok(RuntimeValue::I64(res))
    } else {
        Ok(RuntimeValue::I32(res as u32))
    }
}

/// Implement one LLVM integer/pointer conversion. Returns the produced value in
/// a representation matching `to_ty` (see [`int_binop`] for the sub-32-bit
/// convention).
fn cast_value(
    op: CastOp,
    from_ty: IrType,
    to_ty: IrType,
    value: &RuntimeValue,
) -> Result<RuntimeValue, InterpError> {
    use CastOp::*;
    let from_w = from_ty.integer_width();
    let to_w = to_ty.integer_width();
    match op {
        Zext | Sext | Trunc => {
            let (fw, tw) = match (from_w, to_w) {
                (Some(f), Some(t)) => (f, t),
                _ => {
                    return Err(InterpError::TypeMismatch(format!(
                        "{} requires integer types, got {} -> {}",
                        op.name(),
                        from_ty,
                        to_ty
                    )))
                }
            };
            let bits = int_bits(value, from_ty);
            let out = match op {
                Zext => {
                    if tw <= fw {
                        return Err(InterpError::TypeMismatch(format!(
                            "zext requires widening ({} -> {})",
                            from_ty, to_ty
                        )));
                    }
                    bits
                }
                Sext => {
                    if tw <= fw {
                        return Err(InterpError::TypeMismatch(format!(
                            "sext requires widening ({} -> {})",
                            from_ty, to_ty
                        )));
                    }
                    let sign = fw < 64 && (bits & (1u64 << (fw - 1))) != 0;
                    if sign {
                        bits | (!0u64 << fw)
                    } else {
                        bits
                    }
                }
                Trunc => {
                    if tw >= fw {
                        return Err(InterpError::TypeMismatch(format!(
                            "trunc requires narrowing ({} -> {})",
                            from_ty, to_ty
                        )));
                    }
                    mask_to_width(bits, to_ty)
                }
                _ => unreachable!(),
            };
            Ok(to_width_value(to_ty, out))
        }
        Bitcast => {
            let (fw, tw) = (from_ty.size_in_bytes(), to_ty.size_in_bytes());
            if fw != tw {
                return Err(InterpError::TypeMismatch(format!(
                    "bitcast requires equal byte sizes ({} -> {})",
                    from_ty, to_ty
                )));
            }
            // pointer <-> pointer: address unchanged.
            if matches!(from_ty, IrType::Pointer) && matches!(to_ty, IrType::Pointer) {
                return match value {
                    RuntimeValue::Pointer(a) => Ok(RuntimeValue::Pointer(*a)),
                    _ => Err(InterpError::TypeMismatch("bitcast ptr source not a pointer".into())),
                };
            }
            let bits: u64 = match (from_ty, value) {
                (IrType::F64, RuntimeValue::F64(v)) => v.to_bits(),
                (_, v) if from_ty.is_integer() || from_ty.is_pointer_sized_integer() => int_bits(v, from_ty),
                _ => {
                    return Err(InterpError::TypeMismatch(format!(
                        "bitcast from {} unsupported",
                        from_ty
                    )))
                }
            };
            match to_ty {
                IrType::F64 => Ok(RuntimeValue::F64(f64::from_bits(bits))),
                t if t.is_integer() || t.is_pointer_sized_integer() => Ok(to_width_value(t, bits)),
                IrType::Pointer => Ok(RuntimeValue::Pointer(bits as u32)),
                _ => Err(InterpError::TypeMismatch(format!(
                    "bitcast to {} unsupported",
                    to_ty
                ))),
            }
        }
        PtrToInt => {
            let addr = match value {
                RuntimeValue::Pointer(a) => *a as u64,
                _ => {
                    return Err(InterpError::TypeMismatch(
                        "ptrtoint source must be a pointer".into(),
                    ))
                }
            };
            match to_ty {
                IrType::I32 => Ok(RuntimeValue::I32(addr as u32)),
                IrType::I64 => Ok(RuntimeValue::I64(addr)),
                _ => Err(InterpError::TypeMismatch(format!(
                    "ptrtoint target must be a pointer-sized integer, got {}",
                    to_ty
                ))),
            }
        }
        IntToPtr => {
            let addr = match value {
                RuntimeValue::Pointer(a) => return Ok(RuntimeValue::Pointer(*a)),
                v => int_bits(v, from_ty),
            };
            if from_w.map(|w| w > 32).unwrap_or(false) && addr > u32::MAX as u64 {
                // Value does not fit in a 32-bit pointer: trap as a diagnostic
                // rather than silently truncating a real address.
                return Err(InterpError::TypeMismatch(format!(
                    "inttoptr value {addr:#x} does not fit a 32-bit SA48 pointer"
                )));
            }
            Ok(RuntimeValue::Pointer(addr as u32))
        }
    }
}

/// Represent a masked `bits` value according to `ty` (I1/I8/I16 as dedicated
/// variants, I32 as masked I32, I64 as I64).
fn to_width_value(ty: IrType, bits: u64) -> RuntimeValue {
    match ty {
        IrType::I1 => RuntimeValue::I1(bits & 1 != 0),
        IrType::I8 => RuntimeValue::I32(bits as u32),
        IrType::I16 => RuntimeValue::I32(bits as u32),
        IrType::I32 => RuntimeValue::I32(bits as u32),
        IrType::I64 => RuntimeValue::I64(bits),
        IrType::Pointer => RuntimeValue::Pointer(bits as u32),
        _ => RuntimeValue::I32(bits as u32),
    }
}

fn bytes_to_runtime(ty: &IrType, bytes: &[u8]) -> RuntimeValue {
    match ty {
        IrType::I1 => RuntimeValue::I1(bytes[0] != 0),
        IrType::I8 => RuntimeValue::I8(bytes[0]),
        IrType::I16 => RuntimeValue::I16(u16::from_le_bytes([bytes[0], bytes[1]])),
        IrType::I32 => RuntimeValue::I32(u32::from_le_bytes(bytes.try_into().unwrap())),
        IrType::I64 => RuntimeValue::I64(u64::from_le_bytes(bytes.try_into().unwrap())),
        IrType::F64 => RuntimeValue::F64(f64::from_le_bytes(bytes.try_into().unwrap())),
        IrType::Pointer => RuntimeValue::Pointer(u32::from_le_bytes(bytes.try_into().unwrap())),
        IrType::Void => RuntimeValue::I32(0),
    }
}

/// Serialize `val` for a store of static type `ty`.
///
/// A store writes exactly `ty.size_in_bytes()` bytes in little-endian order.
/// The stored cell may be a wider carrier than `ty` (SAIR sub-32-bit arithmetic
/// results travel in an I32 cell whose low `w` bits are the value, matching the
/// interpreter's carrier convention), so the content is masked to `ty`'s width
/// before serialization — the byte count and the value both derive from the
/// declared store type, never from the carrier variant.
fn store_bytes(val: &RuntimeValue, ty: IrType) -> Result<Vec<u8>, InterpError> {
    match ty {
        IrType::F64 => match val {
            RuntimeValue::F64(v) => Ok(v.to_le_bytes().to_vec()),
            _ => Err(InterpError::TypeMismatch(
                "store f64 of a non-f64 value".into(),
            )),
        },
        IrType::Pointer => {
            let addr = match val {
                RuntimeValue::Pointer(a) | RuntimeValue::I32(a) => *a,
                _ => {
                    return Err(InterpError::TypeMismatch(
                        "store pointer of a non-pointer value".into(),
                    ))
                }
            };
            Ok(addr.to_le_bytes().to_vec())
        }
        ty if ty.is_integer() => {
            let bits = int_bits(val, ty);
            let width = ty.integer_width().unwrap();
            let nbytes = width.div_ceil(8) as usize;
            Ok(bits.to_le_bytes()[..nbytes].to_vec())
        }
        _ => Err(InterpError::TypeMismatch(format!(
            "store of unsupported type {ty}"
        ))),
    }
}

impl core::fmt::Display for RuntimeValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RuntimeValue::I1(v) => write!(f, "i1 {}", if *v { 1 } else { 0 }),
            RuntimeValue::I8(v) => write!(f, "i8 {}", v),
            RuntimeValue::I16(v) => write!(f, "i16 {}", v),
            RuntimeValue::I32(v) => write!(f, "i32 {}", v),
            RuntimeValue::I64(v) => write!(f, "i64 {}", v),
            RuntimeValue::F64(v) => write!(f, "f64 {}", v),
            RuntimeValue::Pointer(v) => write!(f, "ptr 0x{v:x}"),
        }
    }
}
