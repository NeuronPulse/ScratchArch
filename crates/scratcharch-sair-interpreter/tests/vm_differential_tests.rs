//! Regression tests for arithmetic right shift on the VM.
//!
//! The SAIR interpreter is the semantic reference; the same module is lowered
//! to ISA and executed on the VM and the two engines must agree bit-for-bit on
//! the width-masked result. These tests pin the sub-32 (i8/i16) sign-fill
//! behaviour: SAIR `ashr` sign-replicates the *declared-width* sign bit, and
//! the lowerer must extend that sign across the whole (lo, hi) helper pair —
//! bits `w..31` of the low limb as well as the high limb — before the software
//! `__sair_ashr64` helper runs. Before the fix the low limb was left masked to
//! its low `w` bits, so a negative sub-32 value shifted as if logical (e.g.
//! i8 `0x80 >> 3` produced `0x10` on the VM instead of `0xF0`).

use scratcharch_core::value::Value;
use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
use scratcharch_sair_interpreter::{Interpreter, RuntimeValue};
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
