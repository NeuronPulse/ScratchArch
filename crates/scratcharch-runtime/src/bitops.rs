//! Engine-neutral semantics for the LLVM bit-intrinsic families ScratchArch
//! realises (`llvm.bswap`/`llvm.ctpop`/`llvm.ctlz`/`llvm.cttz`).
//!
//! The SAIR interpreter and the ISA VM both evaluate these *pure* functions
//! through [`bit_intrinsic_value`] so the value math cannot drift between
//! engines. (The *name grammar* — which `llvm.<family>.i<N>` maps to which
//! kind and width — is a frontend/backend concern and lives with each engine;
//! the runtime crate deliberately stays free of `llvm.*` string handling, see
//! `docs/specification/RUNTIME.md` §4.)
//!
//! Semantics follow SAIR's no-poison policy and the interpreter reference
//! (`dispatch_llvm_intrinsic`):
//!
//! * the value is masked to `width` bits first;
//! * `ctlz`/`cttz` of zero yield `width` (there is no "poison" zero-undef);
//! * `bswap` reverses only the low `width / 8` bytes (`bswap.i8` is identity);
//! * the result occupies the low `width` bits.

/// The bit-intrinsic families, independent of any LLVM name spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitIntrinsicKind {
    /// Byte-swap: reverse the low `width / 8` bytes of the value.
    ByteReverse,
    /// Count the set bits of the (width-masked) value.
    PopCount,
    /// Count leading zeros of the width-bit value; zero yields `width`.
    CountLeadingZeros,
    /// Count trailing zeros of the width-bit value; zero yields `width`.
    CountTrailingZeros,
}

/// Evaluate a bit intrinsic over a `width`-bit value, returning the result in
/// the low `width` bits. Returns `None` for a width outside `{8, 16, 32, 64}`.
pub fn bit_intrinsic_value(kind: BitIntrinsicKind, width: u32, value: u64) -> Option<u64> {
    if !matches!(width, 8 | 16 | 32 | 64) {
        return None;
    }
    let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
    let x = value & mask;
    let out = match kind {
        BitIntrinsicKind::ByteReverse => {
            let nbytes = u64::from(width / 8);
            let mut out = 0u64;
            for i in 0..nbytes {
                let byte = (x >> (i * 8)) & 0xff;
                out |= byte << ((nbytes - 1 - i) * 8);
            }
            out
        }
        BitIntrinsicKind::PopCount => u64::from(x.count_ones()),
        BitIntrinsicKind::CountLeadingZeros => {
            if x == 0 {
                u64::from(width)
            } else {
                // `x` is masked to `width` bits, so its u64 leading zeros
                // include the (64 - width) padding bits above the width.
                u64::from(x.leading_zeros().saturating_sub(64 - width))
            }
        }
        BitIntrinsicKind::CountTrailingZeros => {
            if x == 0 {
                u64::from(width)
            } else {
                u64::from(x.trailing_zeros())
            }
        }
    };
    Some(out)
}
