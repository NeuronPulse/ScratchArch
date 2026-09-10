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

/// The operand-stack signature of an intrinsic, shared by every backend that
/// drives a call through the runtime registry.
///
/// A ScratchArch backend that resolves runtime calls against the registry at
/// *load* time needs to know, before it runs anything, how many operand-stack
/// cells a call consumes and produces. `arg_words` is the number of `u32`
/// arguments the intrinsic takes (each arrives as one word/cell on the ISA
/// operand stack); `result_words` is the number of cells the result occupies on
/// that stack (0 for a void intrinsic, 1 for the value-returning builtins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicSignature {
    pub arg_words: u8,
    pub result_words: u8,
}

impl IntrinsicSignature {
    const fn word_args(n: u8) -> Self {
        IntrinsicSignature { arg_words: n, result_words: 1 }
    }

    /// A void, argument-free intrinsic (`__scratcharch_abort`/`panic`/`trap`).
    const fn void() -> Self {
        IntrinsicSignature { arg_words: 0, result_words: 0 }
    }
}

/// Registry of available runtime intrinsics.
///
/// The default registry includes the standard ScratchArch runtime routines.
/// Additional intrinsics can be registered at runtime for language-specific
/// extensions or backend-specific helpers.
#[derive(Clone)]
pub struct IntrinsicRegistry {
    intrinsics: HashMap<String, IntrinsicFn>,
    /// Optional per-name operand-stack signature. Builtins carry one; an
    /// externally `register`ed intrinsic without a signature cannot be arity-
    /// resolved by a load-time backend (the interpreter still dispatches it).
    signatures: HashMap<String, IntrinsicSignature>,
}

impl IntrinsicRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self {
            intrinsics: HashMap::new(),
            signatures: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with the standard ScratchArch runtime
    /// intrinsics.
    pub fn with_builtins() -> Self {
        let mut reg = Self::empty();
        reg.register_with_signature("__scratcharch_memcpy", memcpy_intrinsic, IntrinsicSignature::word_args(3));
        reg.register_with_signature("__scratcharch_memmove", memmove_intrinsic, IntrinsicSignature::word_args(3));
        reg.register_with_signature("__scratcharch_memset", memset_intrinsic, IntrinsicSignature::word_args(3));
        reg.register_with_signature("__scratcharch_memcmp", memcmp_intrinsic, IntrinsicSignature::word_args(3));
        reg.register_with_signature("__scratcharch_strlen", strlen_intrinsic, IntrinsicSignature::word_args(1));
        reg.register_with_signature("__scratcharch_strcmp", strcmp_intrinsic, IntrinsicSignature::word_args(2));
        reg.register_with_signature("__scratcharch_strcpy", strcpy_intrinsic, IntrinsicSignature::word_args(2));
        reg.register_with_signature("__scratcharch_strncpy", strncpy_intrinsic, IntrinsicSignature::word_args(3));
        reg.register_with_signature("__scratcharch_abort", abort_intrinsic, IntrinsicSignature::void());
        reg.register_with_signature("__scratcharch_panic", panic_intrinsic, IntrinsicSignature::void());
        reg.register_with_signature("__scratcharch_trap", trap_intrinsic, IntrinsicSignature::void());
        reg
    }

    /// Register a new intrinsic under `name`.
    pub fn register(&mut self, name: impl Into<String>, f: IntrinsicFn) {
        let name = name.into();
        self.intrinsics.insert(name.clone(), f);
        self.signatures.remove(&name);
    }

    /// Register a new intrinsic under `name` with its operand-stack signature.
    pub fn register_with_signature(
        &mut self,
        name: impl Into<String>,
        f: IntrinsicFn,
        signature: IntrinsicSignature,
    ) {
        let name = name.into();
        self.intrinsics.insert(name.clone(), f);
        self.signatures.insert(name, signature);
    }

    /// Look up an intrinsic by name.
    pub fn get(&self, name: &str) -> Option<IntrinsicFn> {
        self.intrinsics.get(name).copied()
    }

    /// Look up the operand-stack signature of a registered intrinsic.
    pub fn signature(&self, name: &str) -> Option<IntrinsicSignature> {
        self.signatures.get(name).copied()
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
