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

    // ------------------------------------------------------------------
    // Raw byte access used by the VM's runtime/intrinsic resolver.
    //
    // The SAIR interpreter runs SART helpers and `llvm.mem*` against its flat
    // byte memory with *no* reserved address: byte address 0 is an ordinary
    // (unseeded, so zero) byte there, distinct from the typed Load/Store null
    // check. To make interpreter and VM agree bit-for-bit on runtime calls, the
    // VM runtime path reads the same flat view — these raw accessors bypass the
    // `NullPointer` reservation that the typed Load8/Store8 ops enforce. Only
    // the runtime resolver uses them; ordinary code never reaches address 0
    // through a typed op. See `docs/design/VM_RUNTIME_GAP_ANALYSIS.md` §5.
    // ------------------------------------------------------------------

    /// Number of addressable bytes (equal to `size()`), for range checks.
    pub fn raw_len(&self) -> usize {
        self.data.len()
    }

    /// Read byte `addr` with interpreter-flat semantics: address 0 is a normal
    /// byte, and only an index past the end is an error.
    pub fn raw_read_byte(&self, addr: u32) -> Result<u8, MemoryError> {
        self.data
            .get(addr as usize)
            .copied()
            .ok_or(MemoryError::OutOfBounds(addr))
    }

    /// Write byte `addr` with interpreter-flat semantics (address 0 allowed).
    pub fn raw_write_byte(&mut self, addr: u32, value: u8) -> Result<(), MemoryError> {
        let slot = self
            .data
            .get_mut(addr as usize)
            .ok_or(MemoryError::OutOfBounds(addr))?;
        *slot = value;
        Ok(())
    }

    /// Copy `n` bytes from `src` to `dst` through an internal temporary, so
    /// overlapping regions behave as-if-through-a-temporary — the semantics the
    /// interpreter's `llvm.memcpy`/`llvm.memmove` dispatch applies. The caller
    /// has already established `dst != 0` (null destination is the typed null
    /// check) and that both ranges fit; this method re-checks bounds and reports
    /// the offending range with `OutOfBounds`.
    pub fn range_copy(&mut self, dst: u32, src: u32, n: usize) -> Result<(), MemoryError> {
        if (src as usize) + n > self.data.len() {
            return Err(MemoryError::OutOfBounds(src));
        }
        if (dst as usize) + n > self.data.len() {
            return Err(MemoryError::OutOfBounds(dst));
        }
        let bytes = self.data[src as usize..src as usize + n].to_vec();
        self.data[dst as usize..dst as usize + n].copy_from_slice(&bytes);
        Ok(())
    }

    /// Fill `n` bytes starting at `dst` with `value` (interpreter-flat rules).
    pub fn range_fill(&mut self, dst: u32, value: u8, n: usize) -> Result<(), MemoryError> {
        if (dst as usize) + n > self.data.len() {
            return Err(MemoryError::OutOfBounds(dst));
        }
        self.data[dst as usize..dst as usize + n].fill(value);
        Ok(())
    }
}
