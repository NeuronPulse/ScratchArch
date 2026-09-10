use scratcharch_core::value::Value;
use scratcharch_core::types::Type;
use scratcharch_runtime::{
    bit_intrinsic_value, BitIntrinsicKind, ByteMemory, IntrinsicRegistry, IntrinsicResult,
    RuntimeError, Value as RuntimeValue,
};
use crate::vm::{Vm, Code, VmError};
use crate::callstack::CallStackError;
use crate::stack::StackError;
use crate::runtime::{MemOp, RuntimeKind, VmRuntimeFn};
use crate::memory::LinearMemory;

impl Vm {
    pub fn step(&mut self) -> Result<(), VmError> {
        let code = &self.functions[self.current_func];
        if self.pc >= code.len() {
            self.running = false;
            return Ok(());
        }

        let instr = &code[self.pc];
        self.pc += 1;

        match *instr {
            Code::ConstI32(v) => {
                self.stack.push(Value::I32(v));
            }
            Code::ConstF64(v) => {
                self.stack.push(Value::F64(v));
            }
            Code::ConstI1(v) => {
                self.stack.push(Value::I1(v));
            }
            Code::Drop => {
                self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)?;
            }
            Code::Dup => {
                let v = self.stack.peek().map_err(|StackError::Underflow| VmError::StackUnderflow)?;
                self.stack.push(*v);
            }
            Code::I32Add => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I32(Value::i32_wrapping_add(a, b)));
            }
            Code::I32Sub => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I32(Value::i32_wrapping_sub(a, b)));
            }
            Code::I32Mul => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I32(Value::i32_wrapping_mul(a, b)));
            }
            Code::I32Div => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                let result = Value::i32_div(a, b).ok_or(VmError::DivisionByZero)?;
                self.stack.push(Value::I32(result));
            }
            Code::I32Rem => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                let result = Value::i32_rem(a, b).ok_or(VmError::DivisionByZero)?;
                self.stack.push(Value::I32(result));
            }
            Code::And => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                self.stack.push(Value::I32(a & b));
            }
            Code::Or => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                self.stack.push(Value::I32(a | b));
            }
            Code::Xor => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                self.stack.push(Value::I32(a ^ b));
            }
            Code::Shl => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                self.stack.push(Value::I32(a << (b & 31)));
            }
            Code::Shr => {
                let b = self.pop_i32()?;
                let a = self.pop_i32()?;
                self.stack.push(Value::I32(a >> (b & 31)));
            }
            Code::Eq => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I1(a == b));
            }
            Code::Lt => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I1(a < b));
            }
            Code::Gt => {
                let b = self.pop_u32()?;
                let a = self.pop_u32()?;
                self.stack.push(Value::I1(a > b));
            }
            Code::Load => {
                let addr = self.pop_address()?;
                let bytes = self.memory.read(addr, Type::I32)
                    .map_err(|e| VmError::MemoryError(e.to_string()))?;
                let val = u32::from_le_bytes(bytes.try_into().unwrap());
                self.stack.push(Value::I32(val));
            }
            Code::Store => {
                let val = match self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)? {
                    Value::I32(v) => v,
                    Value::Pointer(v) => v,
                    other => return Err(VmError::TypeMismatch {
                        expected: "i32 or ptr",
                        found: other.to_string(),
                    }),
                };
                let addr = self.pop_address()?;
                let bytes = val.to_le_bytes();
                self.memory.write(addr, &bytes)
                    .map_err(|e| VmError::MemoryError(e.to_string()))?;
            }
            Code::Load8 => {
                let addr = self.pop_address()?;
                let byte = self.memory.read_byte(addr)
                    .map_err(|e| VmError::MemoryError(e.to_string()))?;
                self.stack.push(Value::I32(byte as u32));
            }
            Code::Store8 => {
                // Stack: (addr, byte). Byte is the low 8 bits of the top cell.
                let byte = (self.pop_u32()? & 0xFF) as u8;
                let addr = self.pop_address()?;
                self.memory.write_byte(addr, byte)
                    .map_err(|e| VmError::MemoryError(e.to_string()))?;
            }
            Code::Alloc => {
                let size = self.pop_i32()?;
                let new_sp = self.memory.alloc_stack(size, self.sp)
                    .map_err(|e| VmError::MemoryError(e.to_string()))?;
                self.sp = new_sp;
                self.stack.push(Value::Pointer(new_sp));
            }
            Code::Jump(target) => {
                self.pc = target;
            }
            Code::Branch(t_target, f_target) => {
                let cond = self.pop_i1()?;
                self.pc = if cond { t_target } else { f_target };
            }
            Code::Call(func_idx) => {
                let meta = self.func_meta(func_idx);
                let saved_len = self
                    .stack
                    .len()
                    .checked_sub(meta.param_cells as usize)
                    .ok_or(VmError::StackUnderflow)?;
                self.call_stack
                    .push(self.current_func, self.pc, self.sp, saved_len, meta.local_count)
                    .map_err(|e| match e {
                        CallStackError::MaxDepthReached => {
                            VmError::MemoryError("call stack overflow".to_string())
                        }
                        CallStackError::Empty => VmError::CallStackEmpty,
                    })?;
                self.current_func = func_idx;
                self.pc = 0;
            }
            Code::CallRuntime(id) => {
                let rtf = self.runtime_fns[id as usize].clone();
                self.exec_call_runtime(&rtf)?;
            }
            Code::Pick(n) => {
                let v = self.stack.get(n as usize)
                    .map_err(|StackError::Underflow| VmError::StackUnderflow)?;
                self.stack.push(*v);
            }
            Code::LocalGet(slot) => {
                let frame = self.call_stack.current()
                    .ok_or(VmError::CallStackEmpty)?;
                let val = *frame.locals.get(slot as usize)
                    .ok_or(VmError::InvalidLocal(slot))?;
                self.stack.push(val);
            }
            Code::LocalSet(slot) => {
                let val = self.stack.pop()
                    .map_err(|StackError::Underflow| VmError::StackUnderflow)?;
                let frame = self.call_stack.current_mut()
                    .ok_or(VmError::CallStackEmpty)?;
                let local = frame.locals.get_mut(slot as usize)
                    .ok_or(VmError::InvalidLocal(slot))?;
                *local = val;
            }
            Code::Return => {
                let meta = self.func_meta(self.current_func).clone();
                match self.call_stack.pop() {
                    Ok(frame) => {
                        let mut cells = Vec::with_capacity(meta.return_cells as usize);
                        for _ in 0..meta.return_cells {
                            cells.push(
                                self.stack.pop()
                                    .map_err(|StackError::Underflow| VmError::StackUnderflow)?,
                            );
                        }
                        cells.reverse();

                        if self.call_stack.depth() == 0 {
                            // Root frame: program ends; keep return value on stack.
                            for cell in cells {
                                self.stack.push(cell);
                            }
                            self.running = false;
                        } else {
                            self.current_func = frame.return_func;
                            self.pc = frame.return_pc;
                            self.sp = frame.saved_sp;
                            self.stack.truncate(frame.saved_stack_len);
                            for cell in cells {
                                self.stack.push(cell);
                            }
                        }
                    }
                    Err(CallStackError::Empty) => {
                        self.running = false;
                    }
                    Err(CallStackError::MaxDepthReached) => unreachable!(),
                }
            }
            Code::Trap => {
                // Terminal, program-declared stop: halt in the distinguished
                // trap state with the trap's location. `pc` was already
                // advanced past the trap, so the trap itself is at `pc - 1`.
                self.running = false;
                return Err(VmError::Trap {
                    function: self.current_func,
                    pc: self.pc - 1,
                });
            }
        }

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), VmError> {
        self.running = true;
        while self.running {
            self.step()?;
        }
        Ok(())
    }

    pub fn run_with_limit(&mut self, max_steps: u64) -> Result<(), VmError> {
        self.running = true;
        for _ in 0..max_steps {
            if !self.running {
                break;
            }
            self.step()?;
        }
        Ok(())
    }

    fn pop_i32(&mut self) -> Result<u32, VmError> {
        match self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)? {
            Value::I32(v) => Ok(v),
            other => Err(VmError::TypeMismatch {
                expected: "i32",
                found: other.to_string(),
            }),
        }
    }

    fn pop_u32(&mut self) -> Result<u32, VmError> {
        match self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)? {
            Value::I32(v) | Value::Pointer(v) => Ok(v),
            // An i1 cell is an integer cell of width 1: cell-level comparison
            // and arithmetic read its 0/1 content. This lets SAIR boolean
            // algebra (Eq/Lt/Gt over i1 flags, e.g. the signed-compare
            // expansion) execute on the VM.
            Value::I1(v) => Ok(v as u32),
            other => Err(VmError::TypeMismatch {
                expected: "i32 or ptr",
                found: other.to_string(),
            }),
        }
    }

    fn pop_i1(&mut self) -> Result<bool, VmError> {
        match self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)? {
            Value::I1(v) => Ok(v),
            other => Err(VmError::TypeMismatch {
                expected: "i1",
                found: other.to_string(),
            }),
        }
    }

    fn pop_address(&mut self) -> Result<u32, VmError> {
        match self.stack.pop().map_err(|StackError::Underflow| VmError::StackUnderflow)? {
            Value::Pointer(v) => Ok(v),
            Value::I32(v) => Ok(v),
            other => Err(VmError::TypeMismatch {
                expected: "ptr or i32",
                found: other.to_string(),
            }),
        }
    }

    /// Pop `n` operand-stack cells and return them in *argument order* (the
    /// bottom-most pushed cell first). Each cell is `u32`-broad: an integer of
    /// any width or a pointer participates, matching the operand-stack value
    /// convention.
    fn pop_cells(&mut self, n: u8) -> Result<Vec<u32>, VmError> {
        let mut cells = Vec::with_capacity(n as usize);
        for _ in 0..n {
            cells.push(self.pop_u32()?);
        }
        // Popping yields top-first; reverse for the pushed (argument) order.
        cells.reverse();
        Ok(cells)
    }

    /// Execute a load-time-resolved runtime/intrinsic call. The surrounding ISA
    /// code has already pushed the call's arguments in the wide-value
    /// convention (high limb on top), so every family pops `arg_words` cells and
    /// pushes `result_words` cells.
    fn exec_call_runtime(&mut self, rtf: &VmRuntimeFn) -> Result<(), VmError> {
        match rtf.kind {
            RuntimeKind::SarbBuiltin => self.exec_sarb_builtin(rtf),
            RuntimeKind::LlvmBit { family, width } => self.exec_llvm_bit(rtf, family, width),
            RuntimeKind::LlvmMem { op, len_words } => self.exec_llvm_mem(rtf, op, len_words),
        }
    }

    /// A SART builtin (`__scratcharch_*`): run the shared registry body over a
    /// flat `ByteMemory` view of VM memory, then push its one-word result. The
    /// builtin is found by name once, at load time; here only the `u32` table
    /// index is used.
    fn exec_sarb_builtin(&mut self, rtf: &VmRuntimeFn) -> Result<(), VmError> {
        let args = self.pop_cells(rtf.arg_words)?;
        let mut mem = VmRawMemory(&mut self.memory);
        let result = IntrinsicRegistry::with_builtins()
            .dispatch(&rtf.name, &mut mem, &args)
            .map_err(runtime_to_vm_error)?;
        if rtf.result_words != 0 {
            let cell = match result {
                IntrinsicResult::Void => {
                    return Err(VmError::RuntimeError(format!(
                        "runtime intrinsic '{}' returned void for a value-returning call",
                        rtf.name
                    )))
                }
                IntrinsicResult::Value(RuntimeValue::U32(v)) => v,
                IntrinsicResult::Value(RuntimeValue::I32(v)) => v as u32,
                IntrinsicResult::Value(RuntimeValue::Pointer(p)) => p,
            };
            self.stack.push(Value::I32(cell));
        }
        Ok(())
    }

    /// An `llvm.bswap/ctpop/ctlz/cttz.iN`: evaluate the shared pure bit-math
    /// leaf. Cells arrive bottom-first, so the value occupies the first
    /// `value_words` cells (low limb first); `ctlz`/`cttz` leave a trailing `i1`
    /// immarg the interpreter reads-and-ignores.
    fn exec_llvm_bit(
        &mut self,
        rtf: &VmRuntimeFn,
        family: BitIntrinsicKind,
        width: u8,
    ) -> Result<(), VmError> {
        let cells = self.pop_cells(rtf.arg_words)?;
        let value_words = usize::from(width).div_ceil(32);
        let mut value: u64 = 0;
        for (i, cell) in cells[..value_words].iter().enumerate() {
            value |= (*cell as u64) << (32 * i);
        }
        let result = bit_intrinsic_value(family, u32::from(width), value).ok_or_else(|| {
            VmError::RuntimeError(format!("unsupported bit-intrinsic width {width}"))
        })?;
        // Wide results push the low limb first (low below, high on top) to match
        // the operand-stack value convention; narrower results push one cell.
        if value_words == 2 {
            self.stack.push(Value::I32(result as u32));
            self.stack.push(Value::I32((result >> 32) as u32));
        } else {
            self.stack.push(Value::I32(result as u32));
        }
        Ok(())
    }

    /// An `llvm.memcpy`/`llvm.memmove`/`llvm.memset`: a flat memory op using the
    /// interpreter's contiguous-range / null-destination / through-a-temporary
    /// rules. Cells arrive bottom-first as `[dst, src/val, len(lo..hi),
    /// isvolatile]`; the trailing immarg is ignored. A zero length is a no-op
    /// (and returns before the null check, matching the interpreter).
    fn exec_llvm_mem(
        &mut self,
        rtf: &VmRuntimeFn,
        op: MemOp,
        len_words: u8,
    ) -> Result<(), VmError> {
        let cells = self.pop_cells(rtf.arg_words)?;
        let dst = cells[0];
        let arg1 = cells[1];
        let mut len: u64 = 0;
        for (i, cell) in cells[2..2 + len_words as usize].iter().enumerate() {
            len |= (*cell as u64) << (32 * i);
        }
        if len == 0 {
            return Ok(());
        }
        if dst == 0 {
            return Err(VmError::RuntimeError(
                "null destination pointer in memory intrinsic".to_string(),
            ));
        }
        let mem_len = self.memory.raw_len() as u64;
        let n = len as usize;
        match op {
            MemOp::Copy | MemOp::Move => {
                let (Some(d_end), Some(s_end)) =
                    ((dst as u64).checked_add(len), (arg1 as u64).checked_add(len))
                else {
                    return Err(VmError::RuntimeError(format!(
                        "memory intrinsic length overflow at destination {dst:#x}"
                    )));
                };
                if d_end > mem_len || s_end > mem_len {
                    return Err(VmError::RuntimeError(format!(
                        "memory access out of bounds at address {dst:#x}"
                    )));
                }
                // Through a temporary: `memmove` is well-defined on overlap and
                // `memcpy` semantics agree where LLVM defines them.
                self.memory
                    .range_copy(dst, arg1, n)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
            }
            MemOp::Set => {
                if (dst as u64).saturating_add(len) > mem_len {
                    return Err(VmError::RuntimeError(format!(
                        "memory access out of bounds at address {dst:#x}"
                    )));
                }
                let byte = (arg1 & 0xFF) as u8;
                self.memory
                    .range_fill(dst, byte, n)
                    .map_err(|e| VmError::RuntimeError(e.to_string()))?;
            }
        }
        Ok(())
    }
}

/// Flat `ByteMemory` view of VM linear memory for the runtime/intrinsic path.
/// It uses the raw (address-0-allowed) accessors so the VM executes the *same*
/// SART bodies over the *same* flat memory the interpreter does.
struct VmRawMemory<'a>(&'a mut LinearMemory);

impl ByteMemory for VmRawMemory<'_> {
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError> {
        self.0
            .raw_read_byte(addr)
            .map_err(|_| RuntimeError::MemoryOutOfBounds { addr })
    }

    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError> {
        self.0
            .raw_write_byte(addr, value)
            .map_err(|_| RuntimeError::MemoryOutOfBounds { addr })
    }
}

/// Map a shared runtime failure onto its VM-visible category. `abort`, `panic`,
/// and `trap` keep distinct variants (so the VM can tell them apart from the
/// ISA `Trap` of `unreachable`); every other failure is a `RuntimeError` with
/// its own message.
fn runtime_to_vm_error(e: RuntimeError) -> VmError {
    match e {
        RuntimeError::Abort => VmError::Abort,
        RuntimeError::Panic { msg } => {
            VmError::Panic(msg.unwrap_or_else(|| "panic intrinsic".to_string()))
        }
        RuntimeError::Trap => VmError::RuntimeTrap,
        other => VmError::RuntimeError(other.to_string()),
    }
}
