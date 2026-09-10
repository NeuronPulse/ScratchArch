//! Load-time resolution of bodyless runtime/intrinsic calls on the ISA VM.
//!
//! The VM's ISA is frozen, so a program that calls a function with no `define`d
//! body still carries an ordinary named ISA `Call`. At **load** time the VM
//! resolves that name against a runtime registry and rewrites the call to an
//! internal [`Code::CallRuntime`](crate::vm::Code) that points at one entry of
//! its runtime table — the same runtime/intrinsic names the SAIR interpreter
//! resolves at run time. Resolution happens once, per name, per program; the
//! execute loop dispatches on the resolved kind (an enum match), never on the
//! name string.
//!
//! The three tiers mirror the interpreter exactly ([`docs/design/VM_RUNTIME_GAP_ANALYSIS.md`](../../../docs/design/VM_RUNTIME_GAP_ANALYSIS.md)):
//!
//! 1. **SART builtins** (`__scratcharch_*`) — resolved through the shared
//!    `scratcharch-runtime` [`IntrinsicRegistry`], whose
//!    [`IntrinsicSignature`] carries the operand-stack arity both engines need.
//!    The shared body runs over a flat `ByteMemory` view of the VM memory, so
//!    interpreter and VM execute the *same* function.
//! 2. **`llvm.mem*` flat ops** — realised with the interpreter's contiguous
//!    range / null-destination / through-a-temporary rules.
//! 3. **`llvm.bswap`/`llvm.ctpop`/`llvm.ctlz`/`llvm.cttz.iN`** — evaluated
//!    through the shared [`bit_intrinsic_value`] leaf.
//!
//! Any other bodyless callee stays a load-time
//! [`VmError::UndefinedFunction`](crate::vm::VmError) — never a silent
//! approximation.

use scratcharch_runtime::{BitIntrinsicKind, IntrinsicRegistry};

/// A flat memory op, mirroring the interpreter's `llvm.memcpy/memmove/memset`
/// dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemOp {
    Copy,
    Move,
    Set,
}

/// How a resolved runtime/intrinsic call executes on the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    /// A SART builtin (`__scratcharch_*`): dispatch the shared registry body
    /// over a flat `ByteMemory` view of VM memory.
    SarbBuiltin,
    /// `llvm.bswap/ctpop/ctlz/cttz.i<N>`: pure value expansion via the shared
    /// bit-math leaf.
    LlvmBit {
        family: BitIntrinsicKind,
        width: u8,
    },
    /// `llvm.memcpy/memmove/memset.<variant>`: a flat memory op with the
    /// interpreter's contiguous-range semantics.
    LlvmMem {
        op: MemOp,
        len_words: u8,
    },
}

/// One entry of the VM's load-time runtime table (its `FnId` is the index).
///
/// `arg_words` is how many operand-stack cells the call consumes (its
/// arguments, including any immarg the interpreter reads but ignores);
/// `result_words` is how many cells the call leaves on the stack (0 for a
/// void intrinsic).
#[derive(Debug, Clone)]
pub struct VmRuntimeFn {
    pub name: String,
    pub kind: RuntimeKind,
    pub arg_words: u8,
    pub result_words: u8,
}

/// Resolve a bodyless callee name into a VM runtime function, or `None` if the
/// name is not one of the runtime/intrinsic functions the interpreter supports.
/// The tiers are checked in the interpreter's dispatch order.
pub fn resolve_runtime(name: &str) -> Option<VmRuntimeFn> {
    if let Some(sig) = IntrinsicRegistry::with_builtins().signature(name) {
        return Some(VmRuntimeFn {
            name: name.to_string(),
            kind: RuntimeKind::SarbBuiltin,
            arg_words: sig.arg_words,
            result_words: sig.result_words,
        });
    }
    resolve_llvm_mem(name)
        .or_else(|| resolve_llvm_bit(name))
}

/// Width of the `len` operand of an `llvm.mem*` call in operand-stack cells:
/// an `i64` length is two limbs, every narrower integer is one.
fn len_words(len_seg: &str) -> u8 {
    if len_seg.starts_with("i64") {
        2
    } else {
        1
    }
}

/// Resolve `llvm.memcpy`/`llvm.memmove`/`llvm.memset` calls whose variant
/// grammar matches the canonical shape the interpreter accepts (`p<d>.p<s>.i<w>`
/// for the two-address ops, `p<d>.i<w>` for `memset`). Variants the interpreter
/// rejects (`llvm.memcpy.inline.*`, …) do not match and stay unresolved.
///
/// The interpreter reads only `dst`/`src`/`len` and ignores the trailing
/// `isvolatile` immarg, but the operand stack still carries it, so the VM pops
/// it too: `arg_words = 3 + len_words`.
fn resolve_llvm_mem(name: &str) -> Option<VmRuntimeFn> {
    if let Some(rest) = name.strip_prefix("llvm.memcpy.") {
        let segs: Vec<&str> = rest.split('.').collect();
        if !(segs.len() == 3 && segs[0].starts_with('p') && segs[1].starts_with('p')) {
            return None;
        }
        return Some(mem_entry(name, MemOp::Copy, len_words(segs[2])));
    }
    if let Some(rest) = name.strip_prefix("llvm.memmove.") {
        let segs: Vec<&str> = rest.split('.').collect();
        if !(segs.len() == 3 && segs[0].starts_with('p') && segs[1].starts_with('p')) {
            return None;
        }
        return Some(mem_entry(name, MemOp::Move, len_words(segs[2])));
    }
    if let Some(rest) = name.strip_prefix("llvm.memset.") {
        let segs: Vec<&str> = rest.split('.').collect();
        if !(segs.len() == 2 && segs[0].starts_with('p')) {
            return None;
        }
        return Some(mem_entry(name, MemOp::Set, len_words(segs[1])));
    }
    None
}

fn mem_entry(name: &str, op: MemOp, len_words: u8) -> VmRuntimeFn {
    VmRuntimeFn {
        name: name.to_string(),
        kind: RuntimeKind::LlvmMem { op, len_words },
        arg_words: 3 + len_words,
        result_words: 0,
    }
}

/// Resolve `llvm.bswap/ctpop/ctlz/cttz.i<N>` where `N ∈ {8, 16, 32, 64}`. The
/// width lives in the name, so the operand-stack arity is known at load time: a
/// `>32`-bit value spans two limbs, and `ctlz`/`cttz` carry an `i1` immarg that
/// clang always emits `false` and the interpreter reads-and-ignores.
fn resolve_llvm_bit(name: &str) -> Option<VmRuntimeFn> {
    let rest = name.strip_prefix("llvm.")?;
    // `<family>.i<width>`; split on the trailing `.iN` so a family with dots
    // would still parse.
    let dot = rest.rfind(".i")?;
    let family = &rest[..dot];
    let width: u8 = rest[dot + 2..].parse().ok()?;
    if !matches!(width, 8 | 16 | 32 | 64) {
        return None;
    }
    let kind = match family {
        "bswap" => BitIntrinsicKind::ByteReverse,
        "ctpop" => BitIntrinsicKind::PopCount,
        "ctlz" => BitIntrinsicKind::CountLeadingZeros,
        "cttz" => BitIntrinsicKind::CountTrailingZeros,
        _ => return None,
    };
    let value_words = if width > 32 { 2 } else { 1 };
    let zero_undef = u8::from(matches!(
        kind,
        BitIntrinsicKind::CountLeadingZeros | BitIntrinsicKind::CountTrailingZeros
    ));
    Some(VmRuntimeFn {
        name: name.to_string(),
        kind: RuntimeKind::LlvmBit { family: kind, width },
        arg_words: value_words + zero_undef,
        result_words: value_words,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> RuntimeKind {
        resolve_runtime(name)
            .unwrap_or_else(|| panic!("{name} must resolve"))
            .kind
    }

    fn arity(name: &str) -> (u8, u8) {
        let r = resolve_runtime(name).unwrap_or_else(|| panic!("{name} must resolve"));
        (r.arg_words, r.result_words)
    }

    #[test]
    fn resolves_sarb_builtins_with_registry_arity() {
        assert_eq!(kind("__scratcharch_strlen"), RuntimeKind::SarbBuiltin);
        assert_eq!(arity("__scratcharch_strlen"), (1, 1));
        assert_eq!(arity("__scratcharch_memcpy"), (3, 1));
        assert_eq!(arity("__scratcharch_strncpy"), (3, 1));
        // abort/panic/trap are void: they consume and produce no cells.
        assert_eq!(arity("__scratcharch_abort"), (0, 0));
        assert_eq!(arity("__scratcharch_panic"), (0, 0));
        assert_eq!(arity("__scratcharch_trap"), (0, 0));
    }

    #[test]
    fn resolves_bit_intrinsics_per_width_and_family() {
        assert_eq!(
            kind("llvm.bswap.i16"),
            RuntimeKind::LlvmBit {
                family: BitIntrinsicKind::ByteReverse,
                width: 16
            }
        );
        assert_eq!(arity("llvm.bswap.i16"), (1, 1));
        // A 64-bit value spans two limbs in and out.
        assert_eq!(arity("llvm.bswap.i64"), (2, 2));
        assert_eq!(arity("llvm.ctpop.i8"), (1, 1));
        // ctlz/cttz carry the is_zero_undef i1 immarg the interpreter ignores.
        assert_eq!(arity("llvm.ctlz.i32"), (2, 1));
        assert_eq!(arity("llvm.cttz.i64"), (3, 2));
    }

    #[test]
    fn resolves_memory_intrinsics_per_variant() {
        assert_eq!(
            kind("llvm.memcpy.p0.p0.i64"),
            RuntimeKind::LlvmMem {
                op: MemOp::Copy,
                len_words: 2
            }
        );
        // dst + src + (i64 len = 2 cells) + isvolatile = 5 cells, void result.
        assert_eq!(arity("llvm.memcpy.p0.p0.i64"), (5, 0));
        assert_eq!(arity("llvm.memmove.p0.p0.i32"), (4, 0));
        assert_eq!(
            kind("llvm.memset.p0.i32"),
            RuntimeKind::LlvmMem {
                op: MemOp::Set,
                len_words: 1
            }
        );
        assert_eq!(arity("llvm.memset.p0.i64"), (5, 0));
    }

    #[test]
    fn rejects_everything_else() {
        // A non-canonical memcpy variant the interpreter also rejects.
        assert!(resolve_runtime("llvm.memcpy.inline.p0.p0.i64").is_none());
        assert!(resolve_runtime("llvm.memcpy.p0.p0").is_none());
        // An unsupported bit-intrinsic family or width.
        assert!(resolve_runtime("llvm.fshl.i32").is_none());
        assert!(resolve_runtime("llvm.bswap.i128").is_none());
        // An unknown name never resolves — it stays a load-time error.
        assert!(resolve_runtime("no_such_runtime_function").is_none());
        assert!(resolve_runtime("__scratcharch_frobnicate").is_none());
    }
}
