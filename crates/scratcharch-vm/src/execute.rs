use scratcharch_core::value::Value;
use scratcharch_core::types::Type;
use crate::vm::{Vm, Code, VmError};
use crate::callstack::CallStackError;
use crate::stack::StackError;

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
}
