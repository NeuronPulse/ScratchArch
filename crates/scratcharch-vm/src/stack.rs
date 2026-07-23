use scratcharch_core::value::Value;

#[derive(Debug, Clone)]
pub struct OperandStack {
    values: Vec<Value>,
}

impl Default for OperandStack {
    fn default() -> Self {
        Self::new()
    }
}

impl OperandStack {
    pub fn new() -> Self {
        OperandStack {
            values: Vec::new(),
        }
    }

    pub fn push(&mut self, val: Value) {
        self.values.push(val);
    }

    pub fn pop(&mut self) -> Result<Value, StackError> {
        self.values.pop().ok_or(StackError::Underflow)
    }

    pub fn get(&self, depth: usize) -> Result<&Value, StackError> {
        let len = self.values.len();
        let idx = len.checked_sub(1 + depth).ok_or(StackError::Underflow)?;
        self.values.get(idx).ok_or(StackError::Underflow)
    }

    pub fn peek(&self) -> Result<&Value, StackError> {
        self.values.last().ok_or(StackError::Underflow)
    }

    pub fn peek_mut(&mut self) -> Result<&mut Value, StackError> {
        self.values.last_mut().ok_or(StackError::Underflow)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError {
    Underflow,
}

impl core::fmt::Display for StackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StackError::Underflow => write!(f, "stack underflow"),
        }
    }
}
