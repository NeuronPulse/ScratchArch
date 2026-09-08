use std::collections::HashMap;
use crate::function::IrFunction;

/// Address of the base of the module's static data segment (LLVM global
/// variables).
///
/// Both execution backends grow their alloca stack downward from the top of a
/// flat byte memory and refuse to allocate below `stack_limit`, so the region
/// below that floor is never stack-allocated. Global data is seeded at this
/// fixed base; `STATIC_DATA_BASE + static_data.image.len()` must stay at or
/// below the runtime `stack_limit` (4096 by default), which each backend checks
/// and reports as an explicit error rather than letting the stack overwrite
/// global data. The base is 8 so that `i64`-aligned globals keep 8-byte
/// alignment relative to address 0 (which remains reserved as null).
pub const STATIC_DATA_BASE: u32 = 8;

/// The module's static data segment (LLVM `global`/`constant` variables).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StaticData {
    /// Byte image of the segment, seeded at [`STATIC_DATA_BASE`].
    pub image: Vec<u8>,
    /// `true` when every data element is 32-bit-word granular — `i32`/`i64`/`ptr`
    /// scalar leaves at 4- or 8-aligned offsets (arrays of these included). Only
    /// then can the 32-bit-word VM backend access the segment exactly. Sub-word
    /// leaves (`i1`/`i8`/`i16`) and byte arrays/strings are interpreter-exact
    /// only; the VM backend rejects such a module with an explicit diagnostic.
    pub word_exact: bool,
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub entry: String,
    /// Static data segment of the module (global variables). Empty by default
    /// for SAIR modules built directly.
    pub static_data: StaticData,
    func_index: HashMap<String, usize>,
}

impl IrModule {
    pub fn new(entry: impl Into<String>) -> Self {
        IrModule {
            functions: Vec::new(),
            entry: entry.into(),
            static_data: StaticData::default(),
            func_index: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, func: IrFunction) {
        self.func_index.insert(func.name.clone(), self.functions.len());
        self.functions.push(func);
    }

    pub fn get_function(&self, name: &str) -> Option<&IrFunction> {
        self.func_index.get(name).map(|&i| &self.functions[i])
    }

    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IrFunction> {
        let idx = *self.func_index.get(name)?;
        self.functions.get_mut(idx)
    }

    pub fn function_index(&self, name: &str) -> Option<usize> {
        self.func_index.get(name).copied()
    }

    pub fn validate(&self) -> Result<(), String> {
        let entry_idx = self.function_index(&self.entry)
            .ok_or_else(|| format!("entry function '{}' not found", self.entry))?;
        for func in &self.functions {
            func.validate()?;
        }
        let _ = entry_idx;
        Ok(())
    }
}
