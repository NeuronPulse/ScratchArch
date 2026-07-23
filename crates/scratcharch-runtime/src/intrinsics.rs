//! Intrinsic registry and dispatcher.
//!
//! All intrinsic names use the `__scratcharch_` prefix and are independent of
//! any frontend or backend naming convention.

use std::collections::HashMap;

use crate::{
    abort, memcpy, memcmp, memset, memmove, strcmp, strcpy, strlen, strncpy, trap, ByteMemory,
    IntrinsicResult, RuntimeError, Value,
};

/// A callable runtime intrinsic.
///
/// The function receives a mutable view of memory and a list of `u32`
/// arguments. Pointers, lengths, and integer arguments are all passed as `u32`
/// values; the intrinsic is responsible for interpreting them correctly.
pub type IntrinsicFn = fn(&mut dyn ByteMemory, &[u32]) -> Result<IntrinsicResult, RuntimeError>;

/// Registry of available runtime intrinsics.
///
/// The default registry includes the standard ScratchArch runtime routines.
/// Additional intrinsics can be registered at runtime for language-specific
/// extensions or backend-specific helpers.
#[derive(Clone)]
pub struct IntrinsicRegistry {
    intrinsics: HashMap<String, IntrinsicFn>,
}

impl IntrinsicRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self {
            intrinsics: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with the standard ScratchArch runtime
    /// intrinsics.
    pub fn with_builtins() -> Self {
        let mut reg = Self::empty();
        reg.register("__scratcharch_memcpy", memcpy_intrinsic);
        reg.register("__scratcharch_memmove", memmove_intrinsic);
        reg.register("__scratcharch_memset", memset_intrinsic);
        reg.register("__scratcharch_memcmp", memcmp_intrinsic);
        reg.register("__scratcharch_strlen", strlen_intrinsic);
        reg.register("__scratcharch_strcmp", strcmp_intrinsic);
        reg.register("__scratcharch_strcpy", strcpy_intrinsic);
        reg.register("__scratcharch_strncpy", strncpy_intrinsic);
        reg.register("__scratcharch_abort", abort_intrinsic);
        reg.register("__scratcharch_panic", panic_intrinsic);
        reg.register("__scratcharch_trap", trap_intrinsic);
        reg
    }

    /// Register a new intrinsic under `name`.
    pub fn register(&mut self, name: impl Into<String>, f: IntrinsicFn) {
        self.intrinsics.insert(name.into(), f);
    }

    /// Look up an intrinsic by name.
    pub fn get(&self, name: &str) -> Option<IntrinsicFn> {
        self.intrinsics.get(name).copied()
    }

    /// Return true if `name` is a known intrinsic.
    pub fn contains(&self, name: &str) -> bool {
        self.intrinsics.contains_key(name)
    }

    /// Iterate over all registered names.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.intrinsics.keys()
    }
}

impl Default for IntrinsicRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Dispatch a call to a built-in intrinsic by name.
///
/// This is a convenience wrapper around [`IntrinsicRegistry::with_builtins`].
pub fn dispatch_intrinsic<M: ByteMemory>(
    name: &str,
    mem: &mut M,
    args: &[u32],
) -> Result<IntrinsicResult, RuntimeError> {
    IntrinsicRegistry::with_builtins().dispatch(name, mem, args)
}

impl IntrinsicRegistry {
    /// Dispatch a call using this registry.
    pub fn dispatch<M: ByteMemory>(
        &self,
        name: &str,
        mem: &mut M,
        args: &[u32],
    ) -> Result<IntrinsicResult, RuntimeError> {
        let f = self.get(name).ok_or(RuntimeError::UnknownIntrinsic(name.to_string()))?;
        f(mem, args)
    }
}

fn expect_args(name: &str, args: &[u32], n: usize) -> Result<(), RuntimeError> {
    if args.len() != n {
        return Err(RuntimeError::BadIntrinsicArgs {
            name: name.to_string(),
            expected: n,
            got: args.len(),
        });
    }
    Ok(())
}

fn memcpy_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_memcpy", args, 3)?;
    let r = memcpy(mem, args[0], args[1], args[2] as usize)?;
    Ok(IntrinsicResult::Value(Value::Pointer(r)))
}

fn memmove_intrinsic(
    mem: &mut dyn ByteMemory,
    args: &[u32],
) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_memmove", args, 3)?;
    let r = memmove(mem, args[0], args[1], args[2] as usize)?;
    Ok(IntrinsicResult::Value(Value::Pointer(r)))
}

fn memset_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_memset", args, 3)?;
    let r = memset(mem, args[0], args[1] as u8, args[2] as usize)?;
    Ok(IntrinsicResult::Value(Value::Pointer(r)))
}

fn memcmp_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_memcmp", args, 3)?;
    let r = memcmp(mem, args[0], args[1], args[2] as usize)?;
    Ok(IntrinsicResult::Value(Value::I32(r)))
}

fn strlen_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_strlen", args, 1)?;
    let r = strlen(mem, args[0])?;
    Ok(IntrinsicResult::Value(Value::U32(r as u32)))
}

fn strcmp_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_strcmp", args, 2)?;
    let r = strcmp(mem, args[0], args[1])?;
    Ok(IntrinsicResult::Value(Value::I32(r)))
}

fn strcpy_intrinsic(mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_strcpy", args, 2)?;
    let r = strcpy(mem, args[0], args[1])?;
    Ok(IntrinsicResult::Value(Value::Pointer(r)))
}

fn strncpy_intrinsic(
    mem: &mut dyn ByteMemory,
    args: &[u32],
) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_strncpy", args, 3)?;
    let r = strncpy(mem, args[0], args[1], args[2] as usize)?;
    Ok(IntrinsicResult::Value(Value::Pointer(r)))
}

fn abort_intrinsic(_mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_abort", args, 0)?;
    abort()?;
    Ok(IntrinsicResult::Void)
}

fn panic_intrinsic(_mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_panic", args, 0)?;
    crate::panic::panic("panic intrinsic")?;
    Ok(IntrinsicResult::Void)
}

fn trap_intrinsic(_mem: &mut dyn ByteMemory, args: &[u32]) -> Result<IntrinsicResult, RuntimeError> {
    expect_args("__scratcharch_trap", args, 0)?;
    trap()?;
    Ok(IntrinsicResult::Void)
}
