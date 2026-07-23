#[derive(Debug, Clone)]
pub struct Frame {
    pub return_func: usize,
    pub return_pc: usize,
    pub saved_sp: u32,
    pub saved_stack_len: usize,
}

#[derive(Debug, Clone)]
pub struct CallStack {
    frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallStackError {
    Empty,
    MaxDepthReached,
}

impl core::fmt::Display for CallStackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CallStackError::Empty => write!(f, "call stack empty"),
            CallStackError::MaxDepthReached => write!(f, "maximum call depth reached"),
        }
    }
}

impl Default for CallStack {
    fn default() -> Self {
        Self::new()
    }
}

impl CallStack {
    pub const MAX_DEPTH: usize = 1024;

    pub fn new() -> Self {
        CallStack {
            frames: Vec::new(),
        }
    }

    pub fn push(&mut self, func: usize, pc: usize, sp: u32, stack_len: usize) -> Result<(), CallStackError> {
        if self.frames.len() >= Self::MAX_DEPTH {
            return Err(CallStackError::MaxDepthReached);
        }
        self.frames.push(Frame {
            return_func: func,
            return_pc: pc,
            saved_sp: sp,
            saved_stack_len: stack_len,
        });
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Frame, CallStackError> {
        self.frames.pop().ok_or(CallStackError::Empty)
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}
