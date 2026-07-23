use scratcharch_core::types::Type;

#[derive(Debug, Clone)]
pub struct LinearMemory {
    data: Vec<u8>,
    stack_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    OutOfBounds(u32),
    NullPointer,
    InvalidAlignment(u32, u32),
    StackOverflow,
}

impl core::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MemoryError::OutOfBounds(addr) => write!(f, "memory access out of bounds at {addr:#x}"),
            MemoryError::NullPointer => write!(f, "null pointer access"),
            MemoryError::InvalidAlignment(addr, align) => write!(f, "unaligned memory access at {addr:#x} (alignment {align})"),
            MemoryError::StackOverflow => write!(f, "stack overflow"),
        }
    }
}

impl LinearMemory {
    pub fn new(size: u32, stack_base: u32) -> Self {
        LinearMemory {
            data: vec![0u8; size as usize],
            stack_limit: stack_base,
        }
    }

    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }

    pub fn read_byte(&self, addr: u32) -> Result<u8, MemoryError> {
        if addr == 0 {
            return Err(MemoryError::NullPointer);
        }
        if addr as usize >= self.data.len() {
            return Err(MemoryError::OutOfBounds(addr));
        }
        Ok(self.data[addr as usize])
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) -> Result<(), MemoryError> {
        if addr == 0 {
            return Err(MemoryError::NullPointer);
        }
        if addr as usize >= self.data.len() {
            return Err(MemoryError::OutOfBounds(addr));
        }
        self.data[addr as usize] = val;
        Ok(())
    }

    pub fn read(&self, addr: u32, ty: Type) -> Result<Vec<u8>, MemoryError> {
        let size = ty.size_in_bytes();
        if addr == 0 {
            return Err(MemoryError::NullPointer);
        }
        if (addr as usize) + (size as usize) > self.data.len() {
            return Err(MemoryError::OutOfBounds(addr));
        }
        let start = addr as usize;
        let end = start + size as usize;
        Ok(self.data[start..end].to_vec())
    }

    pub fn write(&mut self, addr: u32, bytes: &[u8]) -> Result<(), MemoryError> {
        if addr == 0 {
            return Err(MemoryError::NullPointer);
        }
        if (addr as usize) + bytes.len() > self.data.len() {
            return Err(MemoryError::OutOfBounds(addr));
        }
        let start = addr as usize;
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    pub fn alloc_stack(&mut self, size: u32, current_sp: u32) -> Result<u32, MemoryError> {
        let new_sp = current_sp.checked_sub(size).ok_or(MemoryError::StackOverflow)?;
        if new_sp < self.stack_limit {
            return Err(MemoryError::StackOverflow);
        }
        Ok(new_sp)
    }
}
