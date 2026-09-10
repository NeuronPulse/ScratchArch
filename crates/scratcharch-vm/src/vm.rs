use scratcharch_core::instruction::Instruction;
use scratcharch_core::program::Program;
use crate::stack::OperandStack;
use crate::memory::LinearMemory;
use crate::callstack::CallStack;
use crate::runtime::{resolve_runtime, VmRuntimeFn};

#[derive(Debug, Clone)]
pub enum Code {
    ConstI32(u32),
    ConstF64(f64),
    ConstI1(bool),
    Drop,
    Dup,
    I32Add,
    I32Sub,
    I32Mul,
    I32Div,
    I32Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Lt,
    Gt,
    Load,
    Store,
    Load8,
    Store8,
    Alloc,
    Jump(usize),
    Branch(usize, usize),
    Call(usize),
    /// Call a runtime/intrinsic function resolved at load time (no `define`d
    /// body in the program). The `u32` is the index of the call's
    /// [`VmRuntimeFn`] in the VM's runtime table. The ISA is unchanged: only
    /// this private decode variant grows.
    CallRuntime(u32),
    Return,
    Trap,
    Pick(u32),
    LocalGet(u32),
    LocalSet(u32),
}

pub type FuncCode = Vec<Code>;

#[derive(Debug, Clone)]
pub struct FunctionMeta {
    pub param_cells: u32,
    pub return_cells: u32,
    pub local_count: u32,
}

pub struct Vm {
    pub functions: Vec<FuncCode>,
    pub function_meta: Vec<FunctionMeta>,
    pub current_func: usize,
    pub pc: usize,
    pub stack: OperandStack,
    pub call_stack: CallStack,
    pub memory: LinearMemory,
    pub sp: u32,
    pub running: bool,
    /// Runtime/intrinsic functions resolved at load time; a `Code::CallRuntime`
    /// carries the index of its entry here.
    pub runtime_fns: Vec<VmRuntimeFn>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmError {
    StackUnderflow,
    TypeMismatch { expected: &'static str, found: String },
    DivisionByZero,
    MemoryError(String),
    CallStackEmpty,
    UndefinedFunction(String),
    UndefinedLabel(String),
    InvalidAddress,
    InvalidLocal(u32),
    /// A program-declared stop (`Trap` / `unreachable`): execution halted at the
    /// given function index and program counter. Distinct from every
    /// machine-error class above and from a normal return — see
    /// `EXECUTION_MODEL.md` §5.6.
    Trap { function: usize, pc: usize },
    /// The SART `__scratcharch_abort` runtime intrinsic was called.
    Abort,
    /// The SART `__scratcharch_panic` runtime intrinsic was called, carrying its
    /// optional message.
    Panic(String),
    /// The SART `__scratcharch_trap` runtime intrinsic was called. Distinct
    /// from the ISA [`VmError::Trap`] (`unreachable`).
    RuntimeTrap,
    /// Any other runtime/intrinsic failure (out-of-bounds access, unterminated
    /// string, malformed intrinsic call).
    RuntimeError(String),
}

impl core::fmt::Display for VmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VmError::StackUnderflow => write!(f, "stack underflow"),
            VmError::TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {expected}, got {found}")
            }
            VmError::DivisionByZero => write!(f, "division by zero"),
            VmError::MemoryError(msg) => write!(f, "memory error: {msg}"),
            VmError::CallStackEmpty => write!(f, "call stack empty"),
            VmError::UndefinedFunction(name) => write!(f, "undefined function: {name}"),
            VmError::UndefinedLabel(label) => write!(f, "undefined label: {label}"),
            VmError::InvalidAddress => write!(f, "invalid address"),
            VmError::InvalidLocal(slot) => write!(f, "invalid local slot: {slot}"),
            VmError::Trap { function, pc } => {
                write!(f, "program trap at function #{function}, pc {pc}")
            }
            VmError::Abort => write!(f, "runtime abort"),
            VmError::Panic(msg) => write!(f, "runtime panic: {msg}"),
            VmError::RuntimeTrap => write!(f, "runtime trap"),
            VmError::RuntimeError(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

impl Vm {
    pub fn new(memory_size: u32, stack_base: u32) -> Self {
        Vm {
            functions: Vec::new(),
            function_meta: Vec::new(),
            current_func: 0,
            pc: 0,
            stack: OperandStack::new(),
            call_stack: CallStack::new(),
            memory: LinearMemory::new(memory_size, stack_base),
            sp: memory_size,
            running: false,
            runtime_fns: Vec::new(),
        }
    }

    fn ensure_function_slot(&mut self, func_idx: usize) {
        if self.functions.len() <= func_idx {
            self.functions.resize(func_idx + 1, Vec::new());
        }
        if self.function_meta.len() <= func_idx {
            self.function_meta.resize(
                func_idx + 1,
                FunctionMeta {
                    param_cells: 0,
                    return_cells: 1,
                    local_count: 0,
                },
            );
        }
    }

    pub fn func_meta(&self, func_idx: usize) -> &FunctionMeta {
        &self.function_meta[func_idx]
    }

    pub fn load_program(&mut self, program: &Program) -> Result<(), VmError> {
        let entry_idx = program.get_function_index(&program.entry)
            .ok_or_else(|| VmError::UndefinedFunction(program.entry.clone()))?;

        for func in &program.functions {
            let func_idx = program.get_function_index(&func.name).unwrap();
            let mut code = Vec::with_capacity(func.instructions.len());
            for (_, instr) in &func.instructions {
                let c = match instr {
                    Instruction::ConstI32(v) => Code::ConstI32(*v),
                    Instruction::ConstF64(v) => Code::ConstF64(*v),
                    Instruction::ConstI1(v) => Code::ConstI1(*v),
                    Instruction::Drop => Code::Drop,
                    Instruction::Dup => Code::Dup,
                    Instruction::I32Add => Code::I32Add,
                    Instruction::I32Sub => Code::I32Sub,
                    Instruction::I32Mul => Code::I32Mul,
                    Instruction::I32Div => Code::I32Div,
                    Instruction::I32Rem => Code::I32Rem,
                    Instruction::And => Code::And,
                    Instruction::Or => Code::Or,
                    Instruction::Xor => Code::Xor,
                    Instruction::Shl => Code::Shl,
                    Instruction::Shr => Code::Shr,
                    Instruction::Eq => Code::Eq,
                    Instruction::Lt => Code::Lt,
                    Instruction::Gt => Code::Gt,
                    Instruction::Load => Code::Load,
                    Instruction::Store => Code::Store,
                    Instruction::Load8 => Code::Load8,
                    Instruction::Store8 => Code::Store8,
                    Instruction::Alloc => Code::Alloc,
                    Instruction::Jump(l) => {
                        let target = func.get_label_index(l)
                            .ok_or_else(|| VmError::UndefinedLabel(l.clone()))?;
                        Code::Jump(target)
                    }
                    Instruction::Branch(t, f) => {
                        let tt = func.get_label_index(t)
                            .ok_or_else(|| VmError::UndefinedLabel(t.clone()))?;
                        let ff = func.get_label_index(f)
                            .ok_or_else(|| VmError::UndefinedLabel(f.clone()))?;
                        Code::Branch(tt, ff)
                    }
                    Instruction::Call(name) => {
                        match program.get_function_index(name) {
                            Some(target) => Code::Call(target),
                            // No `define`d body: resolve the name against the
                            // shared runtime/intrinsic registry once, at load
                            // time. Unresolvable names are rejected here — never
                            // silently approximated at run time.
                            None => {
                                let Some(rtf) = resolve_runtime(name) else {
                                    return Err(VmError::UndefinedFunction(name.clone()));
                                };
                                let id = match self.runtime_fns.iter().position(|r| r.name == *name) {
                                    Some(i) => i as u32,
                                    None => {
                                        let i = self.runtime_fns.len() as u32;
                                        self.runtime_fns.push(rtf);
                                        i
                                    }
                                };
                                Code::CallRuntime(id)
                            }
                        }
                    }
                    Instruction::Return => Code::Return,
                    Instruction::Trap => Code::Trap,
                    Instruction::Pick(n) => Code::Pick(*n),
                    Instruction::LocalGet(n) => Code::LocalGet(*n),
                    Instruction::LocalSet(n) => Code::LocalSet(*n),
                };
                code.push(c);
            }
            self.ensure_function_slot(func_idx);
            self.functions[func_idx] = code;
            self.function_meta[func_idx] = FunctionMeta {
                param_cells: func.param_cells,
                return_cells: func.return_cells,
                local_count: func.local_count,
            };
        }

        self.current_func = entry_idx;
        self.pc = 0;
        let entry_meta = self.func_meta(entry_idx);
        self.call_stack
            .push(entry_idx, 0, self.sp, self.stack.len(), entry_meta.local_count)
            .map_err(|_| VmError::CallStackEmpty)?;
        self.running = true;
        Ok(())
    }
}
