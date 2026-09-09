use std::path::PathBuf;

use scratcharch_driver::{CompileConfig, CompileDriver, ExecutionBackend, ExecutionValue, OptLevel};
use scratcharch_target::profile::{AbiVersion, Endianness, IntegerModel, MemoryModel, TargetProfile};

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

/// The VM lowers to a 32-bit-word ISA. 64-bit integers split into two 32-bit
/// limbs for add/sub/compare/cast/load/store (see the multi-cell tests below);
/// i64 multiply and unsigned divide/remainder lower to software helpers
/// (`__sair_mul64`, `__sair_udivrem64`) built from the 32-bit word ops, so real
/// clang i64 mul/udiv/urem and (via the translator's signed expansion) sdiv/srem
/// all run on the VM too. The VM still only carries exactly two limbs per value —
/// profiles that split an i64 differently are rejected. Real clang IR uses i64
/// GEP indices, so the file-based VM corpus is a dedicated set of i32-representable
/// fixtures (`tests/c_programs_vm/`); the full real-clang corpus runs on the SAIR
/// interpreter.
fn c_programs_vm_dir() -> PathBuf {
    project_root().join("tests").join("c_programs_vm")
}

fn run_llvm_file_vm(path: &str, opt_level: OptLevel) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level,
        backend: ExecutionBackend::Vm,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    driver
        .compile_and_run_file(path)
        .map_err(|e| e.to_string())
}

fn run_llvm_file_interp(path: &str, opt_level: OptLevel) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level,
        backend: ExecutionBackend::Interpreter,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    driver
        .compile_and_run_file(path)
        .map_err(|e| e.to_string())
}

fn assert_i32(result: Result<Option<ExecutionValue>, String>, expected: u32, name: &str) {
    match result {
        Ok(Some(ExecutionValue::I32(v))) => assert_eq!(v, expected, "{name}"),
        other => panic!("{name}: expected I32({expected}), got {other:?}"),
    }
}

#[test]
fn test_vm_backend_hello() {
    let path = c_programs_vm_dir().join("hello.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "hello");
}

#[test]
fn test_vm_backend_add() {
    let path = c_programs_vm_dir().join("add.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "add");
}

#[test]
fn test_vm_backend_factorial() {
    let path = c_programs_vm_dir().join("factorial.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 120, "factorial");
}

#[test]
fn test_vm_backend_fib() {
    let path = c_programs_vm_dir().join("fib.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 55, "fib");
}

#[test]
fn test_vm_backend_recursion() {
    let path = c_programs_vm_dir().join("recursion.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 15, "recursion");
}

#[test]
fn test_vm_backend_array() {
    let path = c_programs_vm_dir().join("array.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "array");
}

#[test]
fn test_vm_backend_pointer() {
    let path = c_programs_vm_dir().join("pointer.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "pointer");
}

#[test]
fn test_vm_backend_no_opt() {
    let path = c_programs_vm_dir().join("factorial.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::None), 120, "factorial no-opt");
}

/// A loop-carried `phi` (sum 1..10) lowering through the VM: exercises SAIR
/// `Phi` → ISA phi-elimination edge copies on the backend.
#[test]
fn test_vm_backend_phi_loop() {
    let path = c_programs_vm_dir().join("phi_sum.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 55, "phi_sum");
}

/// A signed-guard loop whose bound is negative: the exact signed `sgt`
/// expansion (select-based) must hold on the VM path too.
#[test]
fn test_vm_backend_negative_signed_loop() {
    let path = c_programs_vm_dir().join("neg_countdown.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 3, "neg_countdown");
}

/// Interpreter agrees with the VM on the negative-signed guard loop.
#[test]
fn test_interp_negative_signed_loop() {
    let path = c_programs_vm_dir().join("neg_countdown.ll");
    assert_i32(run_llvm_file_interp(path.to_str().unwrap(), OptLevel::Basic), 3, "neg_countdown");
}

#[test]
fn test_vm_backend_globals() {
    // Word-granular static data only (i32/i64/ptr, all aligned) — the segment is
    // VM-exact. Interpreter and VM must agree on 47.
    let path = c_programs_vm_dir().join("globals.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 47, "globals (vm)");
    assert_i32(run_llvm_file_interp(path.to_str().unwrap(), OptLevel::Basic), 47, "globals (interp)");
}

/// A static segment with sub-word or byte leaves (strings, i8/i16/i1 data) is
/// interpreter-exact: the VM's memory ops are 32-bit-word granular, so the
/// driver must reject the module with an explicit diagnostic rather than
/// silently misread bytes. `tests/c_programs/globals.ll` (real clang) carries
/// byte data, so the VM path errors while the interpreter runs it exactly.
#[test]
fn test_vm_backend_rejects_byte_granular_globals() {
    let path = project_root().join("tests").join("c_programs").join("globals.ll");
    let result = run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic);
    match result {
        Err(e) => {
            assert!(
                e.contains("sub-word or byte")
                    && e.contains("interpreter-only"),
                "expected the word-granularity diagnostic, got: {e}"
            );
        }
        other => panic!("VM should reject byte-granular globals, got success: {other:?}"),
    }
}

/// A static data segment that would collide with the stack floor is an explicit
/// error, never a silent overwrite: globals live below `stack_limit`, and a
/// segment that does not fit must be reported.
#[test]
fn test_vm_backend_rejects_static_data_above_stack_floor() {
    let config = CompileConfig {
        stack_limit: 16,
        backend: ExecutionBackend::Vm,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    let ir = "
@g = global i32 41
@big = global [64 x i32] zeroinitializer
define i32 @main() {
entry:
  ret i32 0
}
";
    let result = driver.compile_and_run(ir);
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("does not fit below the stack floor"),
                "expected the static-data-overflow diagnostic, got: {msg}"
            );
        }
        other => panic!("VM should reject static data above the stack floor, got: {other:?}"),
    }
}

/// The interpreter applies the same stack-floor bound when seeding the static
/// segment; a segment that does not fit is reported there too, never allowed to
/// collide with the (downward-growing) stack.
#[test]
fn test_interp_rejects_static_data_above_stack_floor() {
    let config = CompileConfig {
        stack_limit: 16,
        backend: ExecutionBackend::Interpreter,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    let ir = "
@g = global i32 41
@big = global [64 x i32] zeroinitializer
define i32 @main() {
entry:
  ret i32 0
}
";
    let result = driver.compile_and_run(ir);
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("StaticDataTooLarge"),
                "expected the interpreter static-data-overflow diagnostic, got: {msg}"
            );
        }
        other => panic!("interpreter should reject static data above the stack floor, got: {other:?}"),
    }
}

#[test]
fn test_vm_backend_invalid_llvm_error() {
    let config = CompileConfig {
        backend: ExecutionBackend::Vm,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    let result = driver.compile_and_run("this is not llvm ir");
    assert!(result.is_err(), "invalid LLVM IR should produce a VM-backend driver error");
}

/// Real clang `signedcmp.c` is pure i32 signed icmp (no memory beyond clang's
/// -O0 param copies): every `icmp slt/sgt/sle/sge` must give the exact signed
/// answer on the VM as well as the interpreter. 59 = the signedcmp.c checksum.
#[test]
fn test_vm_backend_real_clang_signedcmp() {
    let path = project_root().join("tests").join("c_programs").join("signedcmp.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 59, "signedcmp (real clang)");
    assert_i32(run_llvm_file_interp(path.to_str().unwrap(), OptLevel::Basic), 59, "signedcmp (interp)");
}

/// Real clang `i64arith.c` exercises genuine 64-bit add/sub across the limb
/// boundary plus signed i64 compares and a trunc — all two-limb VM lowering, so
/// both execution surfaces must agree on 8. (Unlike the hand-written VM corpus,
/// this is full clang -O0 output with helper-call parameter passing.)
#[test]
fn test_vm_backend_real_clang_i64arith() {
    let path = project_root().join("tests").join("c_programs").join("i64arith.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 8, "i64arith (real clang)");
    assert_i32(run_llvm_file_interp(path.to_str().unwrap(), OptLevel::Basic), 8, "i64arith (interp)");
}

/// Real clang `i64muldiv.c` drives i64 `mul`/`udiv`/`urem`/`sdiv`/`srem`
/// through helper calls at -O0. Each lowers to a software helper on the VM
/// (`__sair_mul64`, `__sair_udivrem64`), so both execution surfaces must agree
/// on the native checksum 3579139508.
#[test]
fn test_vm_backend_real_clang_i64muldiv() {
    let path = project_root().join("tests").join("c_programs").join("i64muldiv.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 3579139508, "i64muldiv (real clang)");
    assert_i32(run_llvm_file_interp(path.to_str().unwrap(), OptLevel::Basic), 3579139508, "i64muldiv (interp)");
}

/// Real clang `array.c` indexes its array with *constant* i64 indices
/// (`getelementptr [5 x i32], ptr %a, i64 0, i64 2`). Those fold to a single
/// i32 byte offset, so even real clang IR can lower all the way to the VM when
/// no value is genuinely 64-bit.
#[test]
fn test_vm_backend_real_clang_array() {
    let path = project_root().join("tests").join("c_programs").join("array.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "array (real clang)");
}

/// A *dynamic* array index forces 64-bit offset arithmetic (clang types array
/// indices `i64`). For a byte array (`scale == 1`) the offset is an i64 `add`,
/// which the VM carries as two 32-bit limbs — the index's low limb addresses the
/// 32-bit pointer space, so dynamic byte indices lower and run. For an array of
/// multi-byte elements (`scale == 4`) scaling the dynamic index is an i64 `mul`,
/// which lowers through the `__sair_mul64` software helper; the scaled index runs
/// on the VM as well (see [`test_vm_backend_dynamic_i64_scaled_index`]). (The
/// SAIR interpreter executes both IRs too.)
#[test]
fn test_vm_backend_dynamic_i64_byte_index() {
    let config = CompileConfig {
        backend: ExecutionBackend::Vm,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    let ir = "
define i32 @main() {
entry:
  %arr = alloca [8 x i8]
  %i = add i64 0, 2
  %p = getelementptr [8 x i8], ptr %arr, i64 0, i64 %i
  store i8 42, ptr %p
  %q = getelementptr [8 x i8], ptr %arr, i64 0, i64 %i
  %v = load i8, ptr %q
  %r = zext i8 %v to i32
  ret i32 %r
}
";
    let result = driver.compile_and_run(ir);
    match result {
        Ok(Some(ExecutionValue::I32(42))) => {}
        other => panic!("dynamic i64 byte-array index should run on the VM, got: {other:?}"),
    }
}

/// Scaling a *dynamic* i64 index by a multi-byte element (`scale == 4`) needs an
/// i64 `mul`. That now lowers through `__sair_mul64`, so this runs on the VM
/// instead of being rejected — interpreter and VM must both return 42.
#[test]
fn test_vm_backend_dynamic_i64_scaled_index() {
    let ir = "
define i32 @main() {
entry:
  %arr = alloca [4 x i32]
  %i = add i64 0, 2
  %p = getelementptr [4 x i32], ptr %arr, i64 0, i64 %i
  store i32 42, ptr %p
  %v = load i32, ptr %p
  ret i32 %v
}
";
    assert_agree(ir, 42, "scaled dynamic i64 index");
}

// ---------------------------------------------------------------------------
// Multi-cell i64 lowering (Part 3): 64-bit integers are two 32-bit limbs on the
// VM. Each IR runs on the SAIR interpreter (the semantic reference) and on the
// VM backend, and the two must agree.
// ---------------------------------------------------------------------------

/// Run inline LLVM IR on the VM backend at Basic opt with `profile`.
fn run_ir_vm_profile(ir: &str, profile: TargetProfile) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level: OptLevel::Basic,
        backend: ExecutionBackend::Vm,
        profile,
        ..CompileConfig::default()
    };
    CompileDriver::new(config)
        .compile_and_run(ir)
        .map_err(|e| e.to_string())
}

fn run_ir_vm(ir: &str) -> Result<Option<ExecutionValue>, String> {
    run_ir_vm_profile(ir, TargetProfile::sa48())
}

/// A `profile` whose cell width equals its pointer width (so pointers stay one
/// cell); `cells_for_type` then splits a 64-bit integer across `div_ceil(64, w)`
/// cells, letting the tests exercise the profile-driven limb count.
fn profile(name: &'static str, cell_width: u32) -> TargetProfile {
    TargetProfile {
        name,
        cell_width,
        pointer_width: cell_width,
        endianness: Endianness::Little,
        integer_model: IntegerModel::ModularWrapping,
        memory_model: MemoryModel::FlatByteAddressable,
        abi_version: AbiVersion::V0_1,
    }
}

/// Assert the interpreter and the VM backend agree on `ir` and both yield
/// `expected` as their exit value.
fn assert_agree(ir: &str, expected: u32, name: &str) {
    let interp = CompileDriver::new(CompileConfig {
        opt_level: OptLevel::Basic,
        backend: ExecutionBackend::Interpreter,
        ..CompileConfig::default()
    })
    .compile_and_run(ir)
    .map_err(|e| e.to_string())
    .unwrap_or_else(|e| panic!("[{name}] interpreter failed: {e}\nIR:\n{ir}"));
    assert_i32(Ok(interp), expected, name);

    assert_i32(run_ir_vm(ir), expected, name);
}

#[test]
fn test_vm_i64_add_carry_across_limbs() {
    // 0xFFFFFFFF + 1 wraps into the high limb; low limb reads back 0.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = add i64 4294967295, 1
  %r = trunc i64 %a to i32
  ret i32 %r
}
",
        0,
        "add low-limb wrap",
    );
    // -1 + 1 => 0: carry out of the all-ones low limb, result all-zero.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = add i64 -1, 1
  %r = trunc i64 %a to i32
  ret i32 %r
}
",
        0,
        "add -1+1",
    );
}

#[test]
fn test_vm_i64_sub_borrow() {
    // 1 - 2 = -1: borrow out of the low limb into the high, whole result -1.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = sub i64 1, 2
  %t = icmp eq i64 %a, -1
  %z = zext i1 %t to i32
  ret i32 %z
}
",
        1,
        "sub borrow",
    );
}

#[test]
fn test_vm_i64_unsigned_compare_crosses_limbs() {
    // 0x1_00000000 > 0xFFFFFFFF is false limb-wise but true over both limbs.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = icmp ugt i64 4294967296, 4294967295
  %r = zext i1 %a to i32
  ret i32 %r
}
",
        1,
        "ugt across limbs",
    );
}

#[test]
fn test_vm_i64_signed_compare_negative() {
    assert_agree(
        "
define i32 @main() {
entry:
  %a = icmp slt i64 -1, 1
  %r = zext i1 %a to i32
  ret i32 %r
}
",
        1,
        "slt -1 < 1",
    );
}

#[test]
fn test_vm_i64_casts() {
    // sext i32 -7 to i64 must reproduce -7 exactly (low limb 0xFFFFFFF9, high
    // limb sign-filled 0xFFFFFFFF) — NOT -1 from corrupting the low limb.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = sext i32 -7 to i64
  %b = add i64 %a, 0
  %c = icmp eq i64 %b, -7
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sext i32 -7 == -7",
    );
    // ...and the low limb truncates back to 0xFFFFFFF9.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = sext i32 -7 to i64
  %lo = trunc i64 %a to i32
  ret i32 %lo
}
",
        4294967289,
        "sext then trunc low limb",
    );
    // zext must leave the high limb zero (0xFFFFFFF9 stays positive).
    assert_agree(
        "
define i32 @main() {
entry:
  %a = zext i32 4294967289 to i64
  %c = icmp eq i64 %a, 4294967289
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "zext high limb zero",
    );
}

#[test]
fn test_vm_i64_memory_roundtrip() {
    // store/load a full i64 and confirm equality over both limbs.
    assert_agree(
        "
define i32 @main() {
entry:
  %p = alloca i64
  store i64 4294967298, ptr %p
  %v = load i64, ptr %p
  %c = icmp eq i64 %v, 4294967298
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "memory i64 0x1_00000002",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %p = alloca i64
  store i64 -1, ptr %p
  %v = load i64, ptr %p
  %c = icmp eq i64 %v, -1
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "memory i64 -1",
    );
    // Distinct non-zero limbs (0x2_00000002) prove no limb is dropped.
    assert_agree(
        "
define i32 @main() {
entry:
  %p = alloca i64
  store i64 8589934594, ptr %p
  %v = load i64, ptr %p
  %c = icmp eq i64 %v, 8589934594
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "memory i64 both limbs distinct",
    );
}

#[test]
fn test_vm_i64_phi_loop() {
    // Loop-carried i64 phi: sum 0..9 == 45, returned as the truncated low limb.
    assert_agree(
        "
define i32 @main() {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %inext, %body ]
  %acc = phi i64 [ 0, %entry ], [ %accnext, %body ]
  %t = icmp slt i64 %i, 10
  br i1 %t, label %body, label %done
body:
  %accnext = add i64 %acc, %i
  %inext = add i64 %i, 1
  br label %loop
done:
  %lo = trunc i64 %acc to i32
  ret i32 %lo
}
",
        45,
        "i64 phi loop sum",
    );
}

#[test]
fn test_vm_i64_select() {
    assert_agree(
        "
define i32 @main() {
entry:
  %s = select i1 true, i64 4294967298, i64 0
  %c = icmp eq i64 %s, 4294967298
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "i64 select true arm",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %s = select i1 false, i64 4294967298, i64 99
  %c = icmp eq i64 %s, 99
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "i64 select false arm",
    );
}

// ---------------------------------------------------------------------------
// Profile-driven limb counts (Part 3): the 64-bit limb split must come from
// `TargetProfile::cells_for_type`, never a hardcoded "i64 == 2 cells". The VM
// word is always 32 bits, so a 64-bit integer lowers when the profile splits it
// into exactly two 32-bit words and is rejected with the profile's real cell
// count otherwise.
// ---------------------------------------------------------------------------

/// A 32-bit-cell profile splits i64 into div_ceil(64, 32) = 2 cells — one word
/// per cell — so two-limb i64 lowering must work unchanged off the SA48 default.
#[test]
fn test_vm_i64_on_sa32_profile() {
    // -1 + 1 wraps across both limbs to 0; eq then proves both limbs agree.
    let ir = "
define i32 @main() {
entry:
  %a = add i64 -1, 1
  %b = icmp eq i64 %a, 0
  %z = zext i1 %b to i32
  ret i32 %z
}
";
    assert_i32(run_ir_vm_profile(ir, profile("sa32", 32)), 1, "i64 add+eq on sa32");
}

/// A 24-bit-cell profile needs div_ceil(64, 24) = 3 cells for an i64; the VM can
/// only carry it as two 32-bit words, so lowering must report the actual cell
/// count instead of silently emitting two limbs. (The store keeps the i64 live so
/// the pass pipeline cannot dead-code it away before lowering.)
#[test]
fn test_vm_i64_rejects_three_cell_profile() {
    let result = run_ir_vm_profile(
        "
define i32 @main() {
entry:
  %p = alloca i64
  store i64 1, ptr %p
  ret i32 0
}
",
        profile("sa24", 24),
    );
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("3 profile cells") && msg.contains("sa24"),
                "expected a diagnostic naming the 3-cell decomposition on sa24, got: {msg}"
            );
        }
        other => panic!("sa24 should reject a 3-cell i64, got success: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// i64 multiply / divide / remainder (Part 2): 64-bit mul/div/rem lower through
// software helpers built from the 32-bit word ISA (`__sair_mul64` for mul,
// `__sair_udivrem64` for unsigned div + rem in one pass). Signed sdiv/srem are
// expanded by the LLVM translator into magnitude-based unsigned div/rem before
// lowering, so they ride the same helpers. Every IR runs on both the SAIR
// interpreter (the semantic reference) and the VM backend and must agree.
// ---------------------------------------------------------------------------

/// Run inline LLVM IR on the SAIR interpreter at Basic opt (helper used by the
/// division-by-zero error-class agreement test, where no exit value exists).
fn run_ir_interp(ir: &str) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level: OptLevel::Basic,
        backend: ExecutionBackend::Interpreter,
        ..CompileConfig::default()
    };
    CompileDriver::new(config)
        .compile_and_run(ir)
        .map_err(|e| e.to_string())
}

#[test]
fn test_vm_i64_mul_full_width() {
    // (2^32 + 1)^2 wraps to 2^33 + 1: result_hi comes from the low-limb pair's
    // 64-bit product carried across the limb boundary.
    assert_agree(
        "
define i32 @main() {
entry:
  %m = mul i64 4294967297, 4294967297
  %c = icmp eq i64 %m, 8589934593
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "mul (2^32+1)^2",
    );
    // 2^32 * (2^32 + 1) wraps to 2^32: result_hi is nonzero solely from the
    // high-limb cross terms (the low-limb product is 0).
    assert_agree(
        "
define i32 @main() {
entry:
  %m = mul i64 4294967296, 4294967297
  %c = icmp eq i64 %m, 4294967296
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "mul 2^32*(2^32+1) high-limb cross",
    );
    // All-ones * 2 wraps to all-ones-minus-one; all-ones squared is 1.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = mul i64 -1, 2
  %c1 = icmp eq i64 %a, -2
  %s = mul i64 -1, -1
  %c2 = icmp eq i64 %s, 1
  %r = and i1 %c1, %c2
  %z = zext i1 %r to i32
  ret i32 %z
}
",
        1,
        "mul -1*2 and -1*-1",
    );
    // A small mul round-trips through the wide helper and truncates to 35.
    assert_agree(
        "
define i32 @main() {
entry:
  %m = mul i64 5, 7
  %r = trunc i64 %m to i32
  ret i32 %r
}
",
        35,
        "mul 5*7 low limb",
    );
}

#[test]
fn test_vm_i64_udiv() {
    // 2^32 / 3 = 1431655765 r 1: a 33-bit dividend crossing the limb boundary.
    assert_agree(
        "
define i32 @main() {
entry:
  %q = udiv i64 4294967296, 3
  %c = icmp eq i64 %q, 1431655765
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "udiv 2^32 / 3",
    );
    // (2^63 - 1) / 2^32 = 2^31 - 1: full-width dividend and divisor.
    assert_agree(
        "
define i32 @main() {
entry:
  %q = udiv i64 9223372036854775807, 4294967296
  %c = icmp eq i64 %q, 2147483647
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "udiv (2^63-1) / 2^32",
    );
    // Divisor larger than dividend: quotient 0 (123 / 2^63).
    assert_agree(
        "
define i32 @main() {
entry:
  %q = udiv i64 123, -9223372036854775808
  %c = icmp eq i64 %q, 0
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "udiv divisor > dividend",
    );
}

#[test]
fn test_vm_i64_urem() {
    // (2^63 - 1) mod 2^32 = 2^32 - 1; all-ones mod 2^32 = 2^32 - 1 too.
    assert_agree(
        "
define i32 @main() {
entry:
  %a = urem i64 9223372036854775807, 4294967296
  %c1 = icmp eq i64 %a, 4294967295
  %b = urem i64 -1, 4294967296
  %c2 = icmp eq i64 %b, 4294967295
  %r = and i1 %c1, %c2
  %z = zext i1 %r to i32
  ret i32 %z
}
",
        1,
        "urem mod 2^32",
    );
    // Divisor larger than dividend keeps the full dividend as the remainder.
    assert_agree(
        "
define i32 @main() {
entry:
  %r = urem i64 123, -9223372036854775808
  %c = icmp eq i64 %r, 123
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "urem divisor > dividend",
    );
    // (2^32 + 4) mod 2^32 = 4, returned as the truncated low limb.
    assert_agree(
        "
define i32 @main() {
entry:
  %r = urem i64 4294967300, 4294967296
  %t = trunc i64 %r to i32
  ret i32 %t
}
",
        4,
        "urem low limb",
    );
}

#[test]
fn test_vm_i64_sdiv_signed() {
    // All four sign combinations truncate toward zero.
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 10, 3
  %t = trunc i64 %q to i32
  ret i32 %t
}
",
        3,
        "sdiv 10 / 3",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 -10, 3
  %c = icmp eq i64 %q, -3
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sdiv -10 / 3",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 10, -3
  %c = icmp eq i64 %q, -3
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sdiv 10 / -3",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 -7, 2
  %c = icmp eq i64 %q, -3
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sdiv -7 / 2",
    );
    // The magnitude edge case that does not overflow: -(2^63-1) / -1.
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 -9223372036854775807, -1
  %c = icmp eq i64 %q, 9223372036854775807
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sdiv -(2^63-1) / -1",
    );
}

/// LLVM calls `sdiv INT64_MIN, -1` undefined behavior (it overflows). This
/// toolchain's arithmetic is wrapping everywhere (see SAIR invariants), so the
/// expansion's magnitude `udiv` of |INT64_MIN| = 2^63 by 1 wraps back to
/// INT64_MIN — the x86-consistent answer. Both engines must agree on it.
#[test]
fn test_vm_i64_sdiv_min_by_neg_one_wraps() {
    assert_agree(
        "
define i32 @main() {
entry:
  %q = sdiv i64 -9223372036854775808, -1
  %c = icmp eq i64 %q, -9223372036854775808
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "sdiv INT64_MIN / -1 wraps to INT64_MIN",
    );
}

#[test]
fn test_vm_i64_srem_signed() {
    // srem takes the sign of the dividend.
    assert_agree(
        "
define i32 @main() {
entry:
  %r = srem i64 -10, 3
  %c = icmp eq i64 %r, -1
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "srem -10 % 3",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %r = srem i64 10, -3
  %c = icmp eq i64 %r, 1
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "srem 10 % -3",
    );
    assert_agree(
        "
define i32 @main() {
entry:
  %r = srem i64 -7, 2
  %c = icmp eq i64 %r, -1
  %z = zext i1 %c to i32
  ret i32 %z
}
",
        1,
        "srem -7 % 2",
    );
    // -10 % 3 == -1, whose low limb is all-ones (as a u32 exit value).
    assert_agree(
        "
define i32 @main() {
entry:
  %r = srem i64 -10, 3
  %t = trunc i64 %r to i32
  ret i32 %t
}
",
        4294967295,
        "srem low limb of -1",
    );
}

/// Division by zero is an error in both engines, never a silent result: the VM's
/// `__sair_udivrem64` helper reaches a manufactured `I32Div`-by-zero (VmError
/// "division by zero"), and the interpreter reports
/// [`InterpError::DivisionByZero`]. Signed forms hit the same error inside the
/// magnitude `udiv` before any sign is reapplied.
#[test]
fn test_vm_i64_division_by_zero_error_agreement() {
    for op in ["udiv", "urem", "sdiv", "srem"] {
        let ir = format!(
            "define i32 @main() {{\n\
             entry:\n\
             \x20 %q = {op} i64 10, 0\n\
             \x20 %t = trunc i64 %q to i32\n\
             \x20 ret i32 %t\n\
             }}\n"
        );
        match run_ir_vm(&ir) {
            Err(e) => assert!(
                e.contains("division by zero"),
                "[{op}] VM should report division by zero, got: {e}"
            ),
            other => panic!("[{op}] VM must reject division by zero, got success: {other:?}"),
        }
        match run_ir_interp(&ir) {
            Err(e) => assert!(
                e.contains("DivisionByZero"),
                "[{op}] interpreter should report DivisionByZero, got: {e}"
            ),
            other => panic!("[{op}] interpreter must reject division by zero, got success: {other:?}"),
        }
    }
}
