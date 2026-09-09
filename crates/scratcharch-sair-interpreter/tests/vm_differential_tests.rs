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
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
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

// ── Trap model ────────────────────────────────────────────────────

#[test]
fn diff_unreachable_traps_both_engines() {
    assert_both_trap();
}
