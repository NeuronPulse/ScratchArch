//! Differential tests: the SAIR interpreter is the semantic reference; each
//! module is also lowered to ISA and executed on the VM. For the bitwise and
//! shift family the two engines must agree bit-for-bit on the width-masked
//! result, including across the 64-bit limb boundary and in the deterministic
//! poison region for shift amounts (amount mod width).
//!
//! `unreachable` is additionally exercised as a trap in *both* engines: the
//! interpreter returns [`InterpError::Trap`], the VM halts in
//! [`VmError::Trap`]. No value comparison is possible for a trap — the two
//! engines agree on the failure mode.

use scratcharch_core::value::Value;
use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::{CastOp, GepIndex};
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
use scratcharch_runtime::RuntimeError;
use scratcharch_sair_interpreter::{InterpError, Interpreter, RuntimeValue};
use scratcharch_vm::vm::Vm;

fn ir_type(width: u32) -> IrType {
    match width {
        1 => IrType::I1,
        8 => IrType::I8,
        16 => IrType::I16,
        32 => IrType::I32,
        64 => IrType::I64,
        _ => unreachable!("unsupported test width {width}"),
    }
}

/// Unsigned bits carried by an interpreter result value.
fn runtime_bits(v: &RuntimeValue) -> u64 {
    match v {
        RuntimeValue::I1(b) => *b as u64,
        RuntimeValue::I8(x) => *x as u64,
        RuntimeValue::I16(x) => *x as u64,
        RuntimeValue::I32(x) => *x as u64,
        RuntimeValue::I64(x) => *x,
        RuntimeValue::F64(_) => panic!("float result in integer test"),
        RuntimeValue::Pointer(x) => *x as u64,
    }
}

/// Unsigned bits of one VM operand-stack cell.
fn cell_bits(v: &Value) -> u64 {
    match v {
        Value::I1(b) => *b as u64,
        Value::I8(x) => *x as u64,
        Value::I16(x) => *x as u64,
        Value::I32(x) => *x as u64,
        Value::Pointer(x) => *x as u64,
        Value::F64(_) => panic!("float cell in integer test"),
    }
}

/// Reconstruct a result from the top of the VM operand stack. A 64-bit result
/// is two 32-bit limbs (low limb below the high limb, high on top).
fn vm_result_bits(vm: &Vm, width: u32) -> u64 {
    if width == 64 {
        let hi = cell_bits(vm.stack.get(0).expect("high limb missing"));
        let lo = cell_bits(vm.stack.get(1).expect("low limb missing"));
        (hi << 32) | lo
    } else {
        cell_bits(vm.stack.peek().expect("no result on stack"))
    }
}

fn width_mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Build a `main` function that returns the value produced by `build`, then run
/// the same module on the interpreter and on the VM and require bit agreement.
fn assert_interp_vm_agree(width: u32, build: impl FnOnce(&mut IrBuilder) -> ValueId) {
    let make = |label: &str| {
        let mut b = IrBuilder::new("main");
        b.start_function("main", ir_type(width));
        b.new_block("entry");
        let id = build(&mut b);
        b.ret(Some(id));
        let module = b.finish();
        module.validate().unwrap_or_else(|e| {
            panic!("[{label}] module {width}-bit did not validate: {e}")
        });
        module
    };

    let module = make("interp");
    let program = IsaLowerer::new().lower(&module).expect("lower failed");
    let mut interp = Interpreter::new(module, 65536, 4096);
    let expected = runtime_bits(
        &interp.run().expect("interpreter failed").expect("no result"),
    );

    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("vm failed");
    let got = vm_result_bits(&vm, width);

    let mask = width_mask(width);
    assert_eq!(
        got & mask,
        expected & mask,
        "width {width} disagrees: interpreter {:#x}, VM {:#x}",
        expected & mask,
        got & mask,
    );
}

/// Build an `unreachable`-terminated `main` and require both engines to trap.
fn assert_both_trap() {
    let build_module = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::Void);
        b.new_block("entry");
        b.unreachable();
        b.finish()
    };

    let program = {
        let module = build_module();
        IsaLowerer::new().lower(&module).expect("lower failed")
    };

    let mut interp = Interpreter::new(build_module(), 65536, 4096);
    match interp.run() {
        Err(InterpError::Trap) => {}
        other => panic!("interpreter must trap on unreachable, got {other:?}"),
    }

    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    match vm.run() {
        Err(scratcharch_vm::vm::VmError::Trap { .. }) => {}
        other => panic!("VM must trap on unreachable, got {other:?}"),
    }
}

// ── Bitwise (and / or / xor) ──────────────────────────────────────

#[test]
fn diff_and_or_xor_i32() {
    assert_interp_vm_agree(32, |b| {
        let l = b.const_i32(0xF0F0_F0F0);
        let r = b.const_i32(0x0F0F_0F0F);
        let a = b.and(IrType::I32, l, r);
        let o = b.or(IrType::I32, l, r);
        let x = b.xor(IrType::I32, a, o);
        let y = b.xor(IrType::I32, x, l);
        b.or(IrType::I32, y, r)
    });
}

#[test]
fn diff_and_i8_masks_high_bits() {
    // Both operands are widened from i8; only the low 8 bits may survive.
    assert_interp_vm_agree(8, |b| {
        let l = b.const_i8(0b1111_0000);
        let r = b.const_i8(0b1100_1100);
        b.and(IrType::I8, l, r)
    });
}

#[test]
fn diff_xor_i16_round_trip() {
    assert_interp_vm_agree(16, |b| {
        let a = b.const_i16(0xABCD);
        let b1 = b.const_i16(0xF00F);
        let x = b.xor(IrType::I16, a, b1);
        b.xor(IrType::I16, x, b1) // == a
    });
}

#[test]
fn diff_and_or_xor_i64_crosses_limbs() {
    assert_interp_vm_agree(64, |b| {
        let l = b.const_i64(0xF0F0_F0F0_F0F0_F0F0);
        let r = b.const_i64(0xFFFF_0000_FFFF_0000);
        let a = b.and(IrType::I64, l, r);
        let o = b.or(IrType::I64, l, r);
        let x = b.xor(IrType::I64, a, o);
        b.and(IrType::I64, x, r)
    });
}

// ── Left shift ────────────────────────────────────────────────────

#[test]
fn diff_shl_i32_defined_amounts() {
    for (val, amt) in [(1u32, 0u32), (1, 1), (1, 31), (0x8000_0001, 1)] {
        assert_interp_vm_agree(32, move |b| {
            let v = b.const_i32(val);
            let a = b.const_i32(amt);
            b.shl(IrType::I32, v, a)
        });
    }
}

#[test]
fn diff_shl_i8_poison_amount_is_mod_width() {
    // Amount 15 is outside i8's defined region; both engines must agree that
    // the effective amount is 15 & 7 == 7.
    assert_interp_vm_agree(8, |b| {
        let v = b.const_i8(0x01);
        let a = b.const_i8(15);
        b.shl(IrType::I8, v, a)
    });
}

#[test]
fn diff_shl_i64_across_limb_boundary() {
    for amt in [0u32, 1, 31, 32, 33, 63, 64, 65] {
        assert_interp_vm_agree(64, move |b| {
            // A single low bit walked across the 32-bit limb boundary.
            let v = b.const_i64(0x0000_0000_0000_0001);
            let a = b.const_i64(u64::from(amt));
            b.shl(IrType::I64, v, a)
        });
    }
}

// ── Logical shift right ───────────────────────────────────────────

#[test]
fn diff_lshr_i32() {
    for amt in [0u32, 1, 31, 32] {
        assert_interp_vm_agree(32, move |b| {
            let v = b.const_i32(0x8000_0001);
            let a = b.const_i32(amt);
            b.lshr(IrType::I32, v, a)
        });
    }
}

#[test]
fn diff_lshr_i64_high_bit_walked_down() {
    for amt in [1u32, 31, 32, 33, 63] {
        assert_interp_vm_agree(64, move |b| {
            let v = b.const_i64(0x8000_0000_0000_0000);
            let a = b.const_i64(u64::from(amt));
            b.lshr(IrType::I64, v, a)
        });
    }
}

// ── Arithmetic shift right (sign fill) ────────────────────────────

#[test]
fn diff_ashr_i32_negative() {
    for amt in [1u32, 4, 31] {
        assert_interp_vm_agree(32, move |b| {
            let v = b.const_i32(0x8000_0000);
            let a = b.const_i32(amt);
            b.ashr(IrType::I32, v, a)
        });
    }
}

#[test]
fn diff_ashr_i8_negative_sign_fills() {
    for (val, amt) in [(0x80u8, 3u8), (0x80, 0), (0x80, 7), (0xF0, 4), (0x40, 4)] {
        assert_interp_vm_agree(8, move |b| {
            let v = b.const_i8(val);
            let a = b.const_i8(amt);
            b.ashr(IrType::I8, v, a)
        });
    }
}

#[test]
fn diff_ashr_i16_negative_sign_fills() {
    for (val, amt) in [(0x8000u16, 12u16), (0x8000, 15), (0x8000, 1), (0x8001, 8), (0x7FFF, 12)] {
        assert_interp_vm_agree(16, move |b| {
            let v = b.const_i16(val);
            let a = b.const_i16(amt);
            b.ashr(IrType::I16, v, a)
        });
    }
}

#[test]
fn diff_shl_i16_wraps_width() {
    for (val, amt) in [(0x8000u16, 1u16), (0x8001, 1), (0xFFFF, 8), (0x0001, 15)] {
        assert_interp_vm_agree(16, move |b| {
            let v = b.const_i16(val);
            let a = b.const_i16(amt);
            b.shl(IrType::I16, v, a)
        });
    }
}

#[test]
fn diff_lshr_i8_high_bit_goes_logical() {
    for amt in [0u8, 3, 7] {
        assert_interp_vm_agree(8, move |b| {
            let v = b.const_i8(0x80);
            let a = b.const_i8(amt);
            b.lshr(IrType::I8, v, a)
        });
    }
}

#[test]
fn diff_ashr_i64_negative_to_all_ones() {
    for amt in [1u32, 33, 63] {
        assert_interp_vm_agree(64, move |b| {
            let v = b.const_i64(0x8000_0000_0000_0000);
            let a = b.const_i64(u64::from(amt));
            b.ashr(IrType::I64, v, a)
        });
    }
}

#[test]
fn diff_ashr_i64_positive_stays_zero_filled() {
    assert_interp_vm_agree(64, |b| {
        let v = b.const_i64(0x0000_0001_0000_0000);
        let a = b.const_i64(40);
        b.ashr(IrType::I64, v, a)
    });
}

// ── Multiply ──────────────────────────────────────────────────────
//
// SAIR mul is wrapping at every width. A 64-bit mul has no ISA primitive (the
// ISA only multiplies 32-bit words), so it lowers to a software helper built
// from word multiplies over 16-bit digits. These cases drive the carry out of
// the low limb, the high-limb product of the low limbs, and the cross terms.

#[test]
fn diff_mul_i64_carry_across_limb_boundary() {
    for (l, r) in [
        // (2^32 + 1)^2 == 2^64 + 2^33 + 1, which wraps to 2^33 + 1: the classic
        // case where result_hi comes solely from the low-limb × low-limb product.
        (0x0000_0001_0000_0001u64, 0x0000_0001_0000_0001u64),
        // 2^32 × 2^32 == 2^64 wraps to 0: low-limb products are all zero.
        (0x0000_0001_0000_0000u64, 0x0000_0001_0000_0000u64),
        // (2^31+1)·3 = 3·2^31 + 3: the +1 low limb must carry into the high limb.
        (0x0000_0000_8000_0001u64, 3u64),
        // Top bit set: high-limb × low-limb cross term dominates result_hi.
        (0x8000_0000_0000_0001u64, 0x8000_0000_0000_0003u64),
        // All-ones × 2 wraps to all-ones-minus-one.
        (0xFFFF_FFFF_FFFF_FFFFu64, 2u64),
        // All-ones squared.
        (0xFFFF_FFFF_FFFF_FFFFu64, 0xFFFF_FFFF_FFFF_FFFFu64),
        // Identity and zero.
        (0u64, 0u64),
        (1u64, 1u64),
        (0xDEAD_BEEF_CAFE_F00Du64, 1u64),
        (0xDEAD_BEEF_CAFE_F00Du64, 0u64),
    ] {
        assert_interp_vm_agree(64, move |b| {
            let x = b.const_i64(l);
            let y = b.const_i64(r);
            b.mul(IrType::I64, x, y)
        });
    }
}

#[test]
fn diff_mul_i64_small_and_sub_word_still_wrap() {
    // i64 cases that stay inside one word still round-trip through the helper.
    for (l, r) in [(5u64, 7u64), (0xFFFF_FFFFu64, 0x1_0000_0001u64)] {
        assert_interp_vm_agree(64, move |b| {
            let x = b.const_i64(l);
            let y = b.const_i64(r);
            b.mul(IrType::I64, x, y)
        });
    }
    // Sub-64 mul is a native word multiply; it must keep wrapping (i32, i16).
    assert_interp_vm_agree(32, |b| {
        let x = b.const_i32(0x8000_0000);
        let y = b.const_i32(2);
        b.mul(IrType::I32, x, y)
    });
    assert_interp_vm_agree(16, |b| {
        let x = b.const_i8(0xFF);
        let y = b.const_i8(0x02);
        b.mul(IrType::I8, x, y)
    });
}

// ── Unsigned divide / remainder ───────────────────────────────────
//
// SAIR div/rem are unsigned. On the VM an i64 div/rem lowers to one shared
// software helper that computes both quotient and remainder by 64-step
// restoring division; div keeps the quotient pair and drops the remainder, rem
// keeps the remainder pair and drops the quotient. Divisors here are never
// zero — division by zero is a separate error-class test in the driver suite.

#[test]
fn diff_udiv_i64_matches_long_division() {
    for (a, d) in [
        (7u64, 2u64),
        (0u64, 1u64),
        (1u64, 1u64),
        (0xFFFF_FFFF_FFFF_FFFFu64, 1u64),
        (0xFFFF_FFFF_FFFF_FFFFu64, 0xFFFF_FFFF_FFFF_FFFFu64),
        // Divisor larger than dividend: quotient 0.
        (123u64, 0x8000_0000_0000_0000u64),
        // A 33-bit dividend crosses the limb boundary mid-long-division.
        (0x1_0000_0000u64, 0x1_0000_0001u64),
        (0xDEAD_BEEF_CAFE_F00Du64, 0x1000_0000u64),
        (0xFFFF_FFFF_FFFF_FFFFu64, 3u64),
        (0x8000_0000_0000_0001u64, 0x0000_0001_0000_0000u64),
        (0x1234_5678_9ABC_DEF0u64, 0xABCD_EF01u64),
        (0xFFFFFFFF_FFFFFFFFu64, 0x0000_0001_0000_0001u64),
    ] {
        assert_interp_vm_agree(64, move |b| {
            let n = b.const_i64(a);
            let d = b.const_i64(d);
            b.div(IrType::I64, n, d)
        });
    }
}

#[test]
fn diff_urem_i64_matches_long_division() {
    for (a, d) in [
        (7u64, 2u64),
        (0u64, 1u64),
        (0xFFFF_FFFF_FFFF_FFFFu64, 2u64),
        // Remainder must survive across the full 64-bit width.
        (0xFFFF_FFFF_FFFF_FFFFu64, 0x1_0000_0000u64),
        (0xDEAD_BEEF_CAFE_F00Du64, 0x1000_0000u64),
        (0x8000_0000_0000_0001u64, 0x0000_0001_0000_0000u64),
        (0x1234_5678_9ABC_DEF0u64, 0xABCD_EF01u64),
        (123u64, 0x8000_0000_0000_0000u64),
    ] {
        assert_interp_vm_agree(64, move |b| {
            let n = b.const_i64(a);
            let d = b.const_i64(d);
            b.rem(IrType::I64, n, d)
        });
    }
}

#[test]
fn diff_divrem_sub_word_unsigned_still_exact() {
    // i32 udiv/urem are native word ops; both engines keep the unsigned answer.
    assert_interp_vm_agree(32, |b| {
        let n = b.const_i32(0x8000_0000);
        let d = b.const_i32(0x1_0000);
        b.div(IrType::I32, n, d)
    });
    assert_interp_vm_agree(32, |b| {
        let n = b.const_i32(0x8000_0001);
        let d = b.const_i32(0x1000);
        b.rem(IrType::I32, n, d)
    });
    assert_interp_vm_agree(8, |b| {
        let n = b.const_i8(0xFF);
        let d = b.const_i8(7);
        b.rem(IrType::I8, n, d)
    });
}

/// A fixed-seed LCG so the sweep is reproducible without an external RNG.
fn lcg(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

/// Randomized i64 mul / udiv / urem agreement sweep. The interpreter computes
/// the reference from Rust `u64` arithmetic; the VM runs the software helpers.
/// Wide pseudo-random operands (plus small and boundary divisors) exercise the
/// full 64-iteration long division and every cross-limb carry in the multiply.
#[test]
fn diff_mul_divrem_i64_random_sweep() {
    let mut state = 0x5EED_2026_0909_0001u64;
    for _ in 0..512 {
        let a = lcg(&mut state);
        let b = lcg(&mut state);
        // Interleave pseudo-random with structurally interesting divisors.
        let d = match b & 7 {
            0 => 1,
            1 => 2,
            2 => b | 1, // odd full-width
            3 => (b >> 1) | 1,
            4 => 0xFFFF_FFFF,
            5 => 0x1_0000_0001,
            _ => b,
        };

        assert_interp_vm_agree(64, move |bb| {
            let x = bb.const_i64(a);
            let y = bb.const_i64(d);
            bb.mul(IrType::I64, x, y)
        });
        assert_interp_vm_agree(64, move |bb| {
            let n = bb.const_i64(a);
            let dd = bb.const_i64(d);
            bb.div(IrType::I64, n, dd)
        });
        assert_interp_vm_agree(64, move |bb| {
            let n = bb.const_i64(a);
            let dd = bb.const_i64(d);
            bb.rem(IrType::I64, n, dd)
        });
    }
}

#[test]
fn diff_mul_i64_boundary_divisors() {
    // Multipliers that force every 16-bit digit product to carry: full words and
    // patterns near 0xFFFFFFFF / 0x10000 boundaries.
    for m in [
        0xFFFF_FFFFu64,
        0x1_0000_0000u64,
        0xFFFF_FFFF_FFFF_FFFFu64,
        0x0000_FFFF_0000_FFFFu64,
        0x0000_0000_FFFF_0000u64,
        0x8000_0000_0000_0000u64,
    ] {
        for a in [0xFFFF_FFFFu64, 0x1_0000_0001u64, 0xDEAD_BEEF_CAFE_F00Du64, 0x7u64] {
            assert_interp_vm_agree(64, move |b| {
                let x = b.const_i64(a);
                let y = b.const_i64(m);
                b.mul(IrType::I64, x, y)
            });
        }
    }
}

// ── Trap model ────────────────────────────────────────────────────

#[test]
fn diff_unreachable_traps_both_engines() {
    assert_both_trap();
}

// ── Reinterpret and byte-width memory (differential) ──────────────
//
// Part 1/2 of the reinterpret + byte-memory slice: these drive the *lowering*
// of pointer reinterpretation and width-accurate loads/stores (i1/i8/i16 by
// byte ops) without the LLVM frontend in between. Pointer addresses are engine
// internals, so every assertion is address-independent: either a stored value
// round-trips through a pointer/integer form, or two integers derived from the
// same pointer are compared within one engine run.

/// An i16 store/load round-trips its full 16-bit value (0xBEEF) on both
/// engines — the store splits into two LE bytes, the load recombines them.
#[test]
fn diff_mem_i16_roundtrip() {
    assert_interp_vm_agree(16, |b| {
        let p = b.alloca(IrType::I16);
        let v = b.const_i16(0xBEEF);
        b.store(IrType::I16, v, p);
        b.load(IrType::I16, p)
    });
}

/// An i8 store/load round-trips a full byte (0xEF) on both engines.
#[test]
fn diff_mem_i8_roundtrip() {
    assert_interp_vm_agree(8, |b| {
        let p = b.alloca(IrType::I8);
        let v = b.const_i8(0xEF);
        b.store(IrType::I8, v, p);
        b.load(IrType::I8, p)
    });
}

/// An i1 store writes one byte and an i1 load reads `byte != 0`: storing true
/// then false round-trips through both flag values.
#[test]
fn diff_mem_i1_store_load_flags() {
    assert_interp_vm_agree(32, |b| {
        let p = b.alloca(IrType::I1);
        let t_true = b.const_i1(true);
        let t_false = b.const_i1(false);
        b.store(IrType::I1, t_true, p);
        let t = b.load(IrType::I1, p);
        b.store(IrType::I1, t_false, p);
        let f = b.load(IrType::I1, p);
        let ok_t = b.eq(IrType::I1, t, t_true);
        let ok_f = b.eq(IrType::I1, f, t_false);
        let ok = b.and(IrType::I1, ok_t, ok_f);
        b.cast(CastOp::Zext, IrType::I1, IrType::I32, ok)
    });
}

/// `ptrtoint ptr -> i32` then `inttoptr i32 -> ptr` is a zero-cost cell copy:
/// the integer form of the address still loads the stored value.
#[test]
fn diff_reinterpret_ptrtoint_inttoptr_i32_roundtrip() {
    assert_interp_vm_agree(32, |b| {
        let p = b.alloca(IrType::I32);
        let v = b.const_i32(300);
        b.store(IrType::I32, v, p);
        let a = b.cast(CastOp::PtrToInt, IrType::Pointer, IrType::I32, p);
        let q = b.cast(CastOp::IntToPtr, IrType::I32, IrType::Pointer, a);
        b.load(IrType::I32, q)
    });
}

/// `ptrtoint ptr -> i64` zero-extends into the `(low, high)` limb pair, and the
/// low limb truncates back to the `i32` address form.
#[test]
fn diff_reinterpret_ptrtoint_i64_zero_extends() {
    assert_interp_vm_agree(32, |b| {
        let p = b.alloca(IrType::I16);
        let a32 = b.cast(CastOp::PtrToInt, IrType::Pointer, IrType::I32, p);
        let a64 = b.cast(CastOp::PtrToInt, IrType::Pointer, IrType::I64, p);
        let lo = b.cast(CastOp::Trunc, IrType::I64, IrType::I32, a64);
        let ok_lo = b.eq(IrType::I32, lo, a32);
        let shift = b.const_i64(32);
        let hi = b.lshr(IrType::I64, a64, shift);
        let hi32 = b.cast(CastOp::Trunc, IrType::I64, IrType::I32, hi);
        let zero = b.const_i32(0);
        let ok_hi = b.eq(IrType::I32, hi32, zero);
        let ok = b.and(IrType::I1, ok_lo, ok_hi);
        b.cast(CastOp::Zext, IrType::I1, IrType::I32, ok)
    });
}

/// `bitcast ptr -> ptr` is a genuine no-op: the cast pointer loads the value.
#[test]
fn diff_reinterpret_bitcast_ptr_noop() {
    assert_interp_vm_agree(32, |b| {
        let p = b.alloca(IrType::I32);
        let v = b.const_i32(90);
        b.store(IrType::I32, v, p);
        let q = b.cast(CastOp::Bitcast, IrType::Pointer, IrType::Pointer, p);
        b.load(IrType::I32, q)
    });
}

/// `inttoptr i64 -> ptr` of an address that does not fit the 32-bit pointer
/// traps in both engines — the VM lowers a `Trap` on a nonzero high limb, the
/// interpreter refuses in `cast_value` — never a silent truncation.
#[test]
fn diff_reinterpret_inttoptr_i64_overflow_traps_both() {
    let build_module = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        // 2^32 as an i64: high limb 1, so it overflows the 32-bit pointer.
        let a = b.const_i64(0x1_0000_0000);
        let q = b.cast(CastOp::IntToPtr, IrType::I64, IrType::Pointer, a);
        let one = b.const_i8(1);
        b.store(IrType::I8, one, q);
        let zero = b.const_i32(0);
        b.ret(Some(zero));
        b.finish()
    };

    let program = {
        let module = build_module();
        IsaLowerer::new().lower(&module).expect("lower failed")
    };

    let mut interp = Interpreter::new(build_module(), 65536, 4096);
    match interp.run() {
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("inttoptr value"),
                "interpreter must refuse an overflowing inttoptr, got: {msg}"
            );
        }
        other => panic!("interpreter must reject an overflowing inttoptr, got: {other:?}"),
    }

    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    match vm.run() {
        Err(scratcharch_vm::vm::VmError::Trap { .. }) => {}
        other => panic!("VM must trap on an overflowing inttoptr, got: {other:?}"),
    }
}

// ── Sign extension after truncation (VM regression) ───────────────
//
// A negative sub-32 value is *canonical* on both engines: after a `trunc`, the
// cell holds only its low `fw` bits (i8 `-64` is `0xC0`, not `0xFFFFFFC0`).
// The VM sign-extends by OR-ing an all-ones fill onto that canonical source.
// Before the fix in `IsaLowerer::emit_sext_low32` the fill was applied raw, so
// `sext(0xC0)` produced `0xC0 | 0xFFFFFFFF = 0xFFFFFFFF` (‑1) instead of
// `0xFFFFFFC0` (‑64). This is exactly the IR shape clang emits for `bitwise.c`
//'s `ashr8` (trunc → sext → ashr → trunc → sext), which returned 255 instead
// of 252 on the VM while the interpreter was right. The interpreter is the
// reference; agreement is asserted bit-for-bit.

#[test]
fn diff_sext_of_truncated_negative_fills_upper_bits_only() {
    // sext(trunc(-64 i32 to i8)) -> i32 must be 0xFFFFFFC0, not all-ones.
    assert_interp_vm_agree(32, |b| {
        let wide = b.const_i32(0xFFFF_FFC0); // -64
        let lo = b.cast(CastOp::Trunc, IrType::I32, IrType::I8, wide);
        b.cast(CastOp::Sext, IrType::I8, IrType::I32, lo)
    });
    // Same through i16: sext(trunc(-4 i32 to i16)) -> 0xFFFFFFFC.
    assert_interp_vm_agree(32, |b| {
        let wide = b.const_i32(0xFFFF_FFFC); // -4
        let lo = b.cast(CastOp::Trunc, IrType::I32, IrType::I16, wide);
        b.cast(CastOp::Sext, IrType::I16, IrType::I32, lo)
    });
    // A directly canonical constant (masked low byte, as a trunc would leave it)
    // hits the same `emit_sext_low32` path.
    assert_interp_vm_agree(32, |b| {
        let lo = b.const_i8(0xC0); // -64
        b.cast(CastOp::Sext, IrType::I8, IrType::I32, lo)
    });
}

#[test]
fn diff_sext_i16_into_i64_low_limb_of_negative() {
    // sext to a 64-bit target also runs `emit_sext_low32` for the low limb
    // (fw < 32), with the sign fill in the high limb.
    assert_interp_vm_agree(64, |b| {
        let wide = b.const_i32(0x8000_0040); // positive high bit, low 16 = 0x0040
        let pos = b.cast(CastOp::Trunc, IrType::I32, IrType::I16, wide);
        let pe = b.cast(CastOp::Sext, IrType::I16, IrType::I64, pos);
        let low = b.const_i32(0xFFFF_FFC0); // -64
        let neg = b.cast(CastOp::Trunc, IrType::I32, IrType::I8, low);
        let ne = b.cast(CastOp::Sext, IrType::I8, IrType::I64, neg);
        // ne == 0xFFFFFFFFFFFFFFC0, pe == 0x0000000000000040; difference keeps
        // the top limb all-ones so a truncated fill would be caught.
        b.sub(IrType::I64, ne, pe)
    });
}

#[test]
fn diff_ashr8_fixture_shape_sext_after_trunc() {
    // Mirror of `ashr8` in tests/c_programs/bitwise.c: the sequence clang emits
    // for `(int)(signed char)((signed char)((signed char)a) >> b)` with a = -64,
    // b = 4. Correct masked result: 252. The pre-fix VM returned 255.
    assert_interp_vm_agree(32, |b| {
        let a = b.const_i32(0xFFFF_FFC0); // -64
        let b_sh = b.const_i32(4);
        let t1 = b.cast(CastOp::Trunc, IrType::I32, IrType::I8, a);
        let x = b.cast(CastOp::Sext, IrType::I8, IrType::I32, t1);
        let s = b.ashr(IrType::I32, x, b_sh);
        let t2 = b.cast(CastOp::Trunc, IrType::I32, IrType::I8, s);
        let y = b.cast(CastOp::Sext, IrType::I8, IrType::I32, t2);
        let ff = b.const_i32(0xFF);
        b.and(IrType::I32, y, ff)
    });
}

// ── Runtime / intrinsic differential coverage ─────────────────────
//
// A bodyless runtime/intrinsic call is resolved once at VM load time: SART
// `__scratcharch_*` builtins run the shared registry body over a flat memory
// view, `llvm.bswap/ctpop/ctlz/cttz.iN` evaluate the shared bit-math leaf, and
// `llvm.mem*` follow the flat-memory rules. The interpreter is the reference;
// every module below runs on both engines and must agree bit-for-bit — or on
// the same failure category, for the abort/panic/trap/div-by-zero families.

/// Interpret `module` and return its result (`Ok(None)` for a void `main`) or
/// the reference error.
fn interp_run(module: IrModule) -> Result<Option<RuntimeValue>, InterpError> {
    Interpreter::new(module, 65536, 4096).run()
}

/// Lower `module`, load it on the VM, run it to completion, and return the VM
/// so the caller can read the result off the operand stack.
fn vm_run(module: &IrModule) -> Result<Vm, scratcharch_vm::vm::VmError> {
    let program = IsaLowerer::new().lower(module).expect("lower failed");
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&program).expect("VM load failed");
    vm.run()?;
    Ok(vm)
}

/// The failure of running `module` on the VM, which `module` is expected to
/// provoke (`Vm` has no `Debug`, so `expect_err` is unavailable).
fn vm_err(module: &IrModule) -> scratcharch_vm::vm::VmError {
    match vm_run(module) {
        Ok(_) => panic!("VM was expected to fail but ran to completion"),
        Err(e) => e,
    }
}

/// Assert `build()` agrees bit-for-bit between the interpreter and the VM at
/// `width`.
fn assert_build_agrees(width: u32, label: &str, build: impl Fn() -> IrModule) {
    let module = build();
    module
        .validate()
        .unwrap_or_else(|e| panic!("{label}: module did not validate: {e}"));
    let expected = runtime_bits(
        &interp_run(build())
            .unwrap_or_else(|e| panic!("{label}: interpreter failed: {e:?}"))
            .expect("interpreter produced no result"),
    );
    let vm = vm_run(&module).unwrap_or_else(|e| panic!("{label}: VM failed: {e:?}"));
    let got = vm_result_bits(&vm, width);
    let mask = width_mask(width);
    assert_eq!(
        got & mask,
        expected & mask,
        "{label}: interpreter {:#x}, VM {:#x}",
        expected & mask,
        got & mask,
    );
}

fn wide_const(b: &mut IrBuilder, width: u32, v: u64) -> ValueId {
    match width {
        8 => b.const_i8(v as u8),
        16 => b.const_i16(v as u16),
        32 => b.const_i32(v as u32),
        64 => b.const_i64(v),
        _ => unreachable!("unsupported test width {width}"),
    }
}

/// Store `bytes` at `base + 0..`.
fn store_bytes(b: &mut IrBuilder, base: ValueId, bytes: &[u8]) {
    for (i, &byte) in bytes.iter().enumerate() {
        let off = b.const_i32(i as u32);
        let p = b.gep(IrType::I8, base, vec![GepIndex::Dynamic(off)]);
        let v = b.const_i8(byte);
        b.store(IrType::I8, v, p);
    }
}

/// An `i32` checksum (byte sum) of the first `n` bytes at `base`.
fn checksum(b: &mut IrBuilder, base: ValueId, n: u32) -> ValueId {
    let mut acc = b.const_i32(0);
    for i in 0..n {
        let off = b.const_i32(i);
        let p = b.gep(IrType::I8, base, vec![GepIndex::Dynamic(off)]);
        let byte = b.load(IrType::I8, p);
        let wide = b.cast(CastOp::Zext, IrType::I8, IrType::I32, byte);
        acc = b.add(IrType::I32, acc, wide);
    }
    acc
}

/// `bswap`/`ctpop`/`ctlz`/`cttz` at every width clang emits (8/16/32/64),
/// including the 64-bit limb boundary and the `ctlz`/`cttz` zero case (`0`
/// yields the width, not poison). `ctlz`/`cttz` carry the `is_zero_undef` i1
/// immarg the interpreter reads-and-ignores; the VM must pop and ignore it too.
#[test]
fn diff_llvm_bit_intrinsics_agree() {
    let samples = |width: u32| -> Vec<u64> {
        match width {
            8 => vec![0, 1, 0x80, 0xAA, 0xFF],
            16 => vec![0, 1, 0x00FF, 0x8000, 0xABCD, 0xFFFF],
            32 => vec![0, 1, 0x8000_0000, 0xFFFF_FFFF, 0x1234_5678],
            _ => vec![0, 1, 0x8000_0000_0000_0000, 0xFFFF_FFFF_FFFF_FFFF, 0x0123_4567_89AB_CDEF],
        }
    };
    for &width in &[8u32, 16, 32, 64] {
        for family in ["bswap", "ctpop", "ctlz", "cttz"] {
            let callee = format!("llvm.{family}.i{width}");
            for v in samples(width) {
                let build = || {
                    let ty = ir_type(width);
                    let mut b = IrBuilder::new("main");
                    b.start_function("main", ty);
                    b.new_block("entry");
                    let val = wide_const(&mut b, width, v);
                    let mut args = vec![val];
                    if family == "ctlz" || family == "cttz" {
                        args.push(b.const_i1(false));
                    }
                    let r = b.call(ty, callee.clone(), args).expect("returns a value");
                    b.ret(Some(r));
                    b.finish()
                };
                assert_build_agrees(width, &format!("{callee} on {v:#x}"), build);
            }
        }
    }
}

/// `llvm.memcpy`/`llvm.memmove`/`llvm.memset` with a *run-time* length (so the
/// translator leaves the call for the VM runtime resolver instead of inlining
/// it). `memmove` copies an overlapping region, exercising the
/// as-if-through-a-temporary rule.
#[test]
fn diff_llvm_mem_intrinsics_agree() {
    let runtime_len = |b: &mut IrBuilder, n: u32| {
        // Two plus (n - 2): a computed value the translator cannot treat as a
        // literal length.
        let two = b.const_i32(2);
        let rest = b.const_i32(n - 2);
        b.add(IrType::I32, two, rest)
    };

    let build_memcpy = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let src = b.alloca_array(IrType::I8, 8);
        store_bytes(&mut b, src, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let dst = b.alloca_array(IrType::I8, 8);
        let len = runtime_len(&mut b, 5);
        let vol = b.const_i1(false);
        b.call(IrType::Void, "llvm.memcpy.p0.p0.i32", vec![dst, src, len, vol]);
        let sum = checksum(&mut b, dst, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "llvm.memcpy runtime length", build_memcpy);

    let build_memmove = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let buf = b.alloca_array(IrType::I8, 8);
        store_bytes(&mut b, buf, &[10, 20, 30, 40, 50, 60, 70, 80]);
        let one = b.const_i32(1);
        let dst = b.gep(IrType::I8, buf, vec![GepIndex::Dynamic(one)]);
        let len = runtime_len(&mut b, 4);
        let vol = b.const_i1(false);
        b.call(IrType::Void, "llvm.memmove.p0.p0.i32", vec![dst, buf, len, vol]);
        let sum = checksum(&mut b, buf, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "llvm.memmove overlap runtime length", build_memmove);

    let build_memset = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let buf = b.alloca_array(IrType::I8, 8);
        let val = b.const_i8(0xAB);
        let len = runtime_len(&mut b, 5);
        let vol = b.const_i1(false);
        b.call(IrType::Void, "llvm.memset.p0.i32", vec![buf, val, len, vol]);
        let sum = checksum(&mut b, buf, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "llvm.memset runtime length", build_memset);
}

/// The SART string/memory builtins: `strlen` returns the count, `strcmp`/
/// `memcmp` the comparison sign, and the writing routines are checked by a
/// destination checksum. The `__scratcharch_mem*` copies use a run-time length
/// so the VM runtime resolver (not the translator's constant-length inlining)
/// executes them.
#[test]
fn diff_sarb_string_and_mem_builtins_agree() {
    let build_strlen = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let buf = b.alloca_array(IrType::I8, 8);
        store_bytes(&mut b, buf, b"hello\0");
        let r = b.call(IrType::I32, "__scratcharch_strlen", vec![buf]).expect("value");
        b.ret(Some(r));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_strlen", build_strlen);

    let build_strcmp = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let a = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, a, b"abc\0");
        let c = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, c, b"abd\0");
        let r = b.call(IrType::I32, "__scratcharch_strcmp", vec![a, c]).expect("value");
        b.ret(Some(r));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_strcmp", build_strcmp);

    let build_strcpy = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let dst = b.alloca_array(IrType::I8, 8);
        let src = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, src, b"hi\0");
        b.call(IrType::Pointer, "__scratcharch_strcpy", vec![dst, src]);
        let sum = checksum(&mut b, dst, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_strcpy", build_strcpy);

    let build_strncpy = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let dst = b.alloca_array(IrType::I8, 8);
        let src = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, src, b"hi\0");
        let n = b.const_i32(6);
        b.call(IrType::Pointer, "__scratcharch_strncpy", vec![dst, src, n]);
        let sum = checksum(&mut b, dst, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_strncpy", build_strncpy);

    let build_memcmp = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let a = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, a, &[1, 2, 3, 4]);
        let c = b.alloca_array(IrType::I8, 4);
        store_bytes(&mut b, c, &[1, 2, 9, 4]);
        let n = {
            let two = b.const_i32(2);
            let rest = b.const_i32(2);
            b.add(IrType::I32, two, rest)
        };
        let r = b.call(IrType::I32, "__scratcharch_memcmp", vec![a, c, n]).expect("value");
        b.ret(Some(r));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_memcmp", build_memcmp);

    let build_runtime_memcpy = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let src = b.alloca_array(IrType::I8, 8);
        store_bytes(&mut b, src, &[3, 1, 4, 1, 5, 9, 2, 6]);
        let dst = b.alloca_array(IrType::I8, 8);
        let len = {
            let two = b.const_i32(2);
            let rest = b.const_i32(3);
            b.add(IrType::I32, two, rest)
        };
        b.call(IrType::Pointer, "__scratcharch_memcpy", vec![dst, src, len]);
        let sum = checksum(&mut b, dst, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_memcpy runtime length", build_runtime_memcpy);

    let build_runtime_memset = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let buf = b.alloca_array(IrType::I8, 8);
        let val = b.const_i32(7);
        let len = {
            let two = b.const_i32(2);
            let rest = b.const_i32(3);
            b.add(IrType::I32, two, rest)
        };
        b.call(IrType::Pointer, "__scratcharch_memset", vec![buf, val, len]);
        let sum = checksum(&mut b, buf, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_memset runtime length", build_runtime_memset);

    let build_runtime_memmove = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let buf = b.alloca_array(IrType::I8, 8);
        store_bytes(&mut b, buf, &[9, 8, 7, 6, 5, 4, 3, 2]);
        let one = b.const_i32(1);
        let dst = b.gep(IrType::I8, buf, vec![GepIndex::Dynamic(one)]);
        let len = {
            let two = b.const_i32(2);
            let rest = b.const_i32(2);
            b.add(IrType::I32, two, rest)
        };
        b.call(IrType::Pointer, "__scratcharch_memmove", vec![dst, buf, len]);
        let sum = checksum(&mut b, buf, 8);
        b.ret(Some(sum));
        b.finish()
    };
    assert_build_agrees(32, "__scratcharch_memmove overlap runtime length", build_runtime_memmove);
}

/// `abort`/`panic`/`trap` keep *distinct* failure categories on both engines,
/// and the runtime trap stays distinct from the ISA `unreachable` trap
/// (`InterpError::Trap` / `VmError::Trap`).
#[test]
fn diff_runtime_failure_categories_distinct() {
    let void_call = |callee: &'static str| {
        move || {
            let mut b = IrBuilder::new("main");
            b.start_function("main", IrType::Void);
            b.new_block("entry");
            b.call(IrType::Void, callee, vec![]);
            b.ret(None);
            b.finish()
        }
    };

    let abort = void_call("__scratcharch_abort");
    let err = interp_run(abort()).unwrap_err();
    assert!(
        matches!(err, InterpError::Runtime(RuntimeError::Abort)),
        "interpreter abort: {err:?}"
    );
    let err = vm_err(&abort());
    assert!(
        matches!(err, scratcharch_vm::vm::VmError::Abort),
        "VM abort: {err:?}"
    );

    let panic = void_call("__scratcharch_panic");
    let err = interp_run(panic()).unwrap_err();
    assert!(
        matches!(err, InterpError::Runtime(RuntimeError::Panic { .. })),
        "interpreter panic: {err:?}"
    );
    let err = vm_err(&panic());
    assert!(
        matches!(err, scratcharch_vm::vm::VmError::Panic(_)),
        "VM panic: {err:?}"
    );

    let trap = void_call("__scratcharch_trap");
    let err = interp_run(trap()).unwrap_err();
    assert!(
        matches!(err, InterpError::Runtime(RuntimeError::Trap)),
        "interpreter runtime trap: {err:?}"
    );
    let err = vm_err(&trap());
    assert!(
        matches!(err, scratcharch_vm::vm::VmError::RuntimeTrap),
        "VM runtime trap: {err:?}"
    );
    // Distinct from the `unreachable` ISA trap on the VM.
    assert!(
        !matches!(err, scratcharch_vm::vm::VmError::Trap { .. }),
        "runtime trap must not collapse into the unreachable trap"
    );
}

/// An unknown bodyless callee is rejected by both engines — never silently
/// approximated. The VM rejects it at *load* time ([`VmError::UndefinedFunction`]);
/// the interpreter rejects it when the call is reached (an unknown intrinsic).
#[test]
fn diff_unknown_runtime_call_rejected_by_both_engines() {
    let build = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let r = b
            .call(IrType::I32, "no_such_runtime_function", vec![])
            .expect("returns a value");
        b.ret(Some(r));
        b.finish()
    };

    let err = interp_run(build()).unwrap_err();
    assert!(
        matches!(
            err,
            InterpError::Runtime(RuntimeError::UnknownIntrinsic(ref n))
                if n == "no_such_runtime_function"
        ),
        "interpreter unknown callee: {err:?}"
    );

    let program = IsaLowerer::new().lower(&build()).expect("lower failed");
    let mut vm = Vm::new(65536, 4096);
    match vm.load_program(&program) {
        Err(scratcharch_vm::vm::VmError::UndefinedFunction(n)) => {
            assert_eq!(n, "no_such_runtime_function");
        }
        other => panic!("VM must reject the unknown callee at load time, got {other:?}"),
    }
}

/// Division by zero keeps its own category on both engines.
#[test]
fn diff_division_by_zero_category() {
    let build = || {
        let mut b = IrBuilder::new("main");
        b.start_function("main", IrType::I32);
        b.new_block("entry");
        let one = b.const_i32(1);
        let zero = b.const_i32(0);
        let q = b.div(IrType::I32, one, zero);
        b.ret(Some(q));
        b.finish()
    };
    let err = interp_run(build()).unwrap_err();
    assert!(
        matches!(err, InterpError::DivisionByZero),
        "interpreter div0: {err:?}"
    );
    let err = vm_err(&build());
    assert!(
        matches!(err, scratcharch_vm::vm::VmError::DivisionByZero),
        "VM div0: {err:?}"
    );
}
