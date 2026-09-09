use std::path::PathBuf;
use std::fs;

use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_sair_interpreter::RuntimeValue;

fn run_llvm_file(path: &str) -> Result<Option<RuntimeValue>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path, e))?;
    let module = translate_llvm(&content).map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp.run().map_err(|e| format!("{:?}", e))
}

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // scratcharch-llvm is at crates/scratcharch-llvm/, tests/ is at tests/c_programs/
    p.pop(); p.pop(); // up to project root
    p
}

fn c_programs_dir() -> PathBuf {
    project_root().join("tests").join("c_programs")
}

#[test]
fn test_pipeline_hello() {
    let path = c_programs_dir().join("hello.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 42),
        other => panic!("hello.ll: expected I32(42), got {:?}", other),
    }
}

#[test]
fn test_pipeline_add() {
    let path = c_programs_dir().join("add.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 42),
        other => panic!("add.ll: expected I32(42), got {:?}", other),
    }
}

#[test]
fn test_pipeline_factorial() {
    let path = c_programs_dir().join("factorial.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 120),
        other => panic!("factorial.ll: expected I32(120), got {:?}", other),
    }
}

#[test]
fn test_pipeline_fib() {
    let path = c_programs_dir().join("fib.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 55),
        other => panic!("fib.ll: expected I32(55), got {:?}", other),
    }
}

#[test]
fn test_pipeline_array() {
    let path = c_programs_dir().join("array.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 42),
        other => panic!("array.ll: expected I32(42), got {:?}", other),
    }
}

#[test]
fn test_pipeline_struct() {
    let path = c_programs_dir().join("struct.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 30),
        other => panic!("struct.ll: expected I32(30), got {:?}", other),
    }
}

#[test]
fn test_pipeline_pointer() {
    let path = c_programs_dir().join("pointer.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 42),
        other => panic!("pointer.ll: expected I32(42), got {:?}", other),
    }
}

#[test]
fn test_pipeline_string() {
    let path = c_programs_dir().join("string.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 5),
        other => panic!("string.ll: expected I32(5), got {:?}", other),
    }
}

#[test]
fn test_pipeline_memory() {
    let path = c_programs_dir().join("memory.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 6),
        other => panic!("memory.ll: expected I32(6), got {:?}", other),
    }
}

#[test]
fn test_pipeline_recursion() {
    let path = c_programs_dir().join("recursion.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 15),
        other => panic!("recursion.ll: expected I32(15), got {:?}", other),
    }
}

/// Real clang output for `__builtin_bswap16/32`, `__builtin_popcount`,
/// `__builtin_clz/ctz`, and the `*ll` 64-bit variants. clang -O0 emits the four
/// bit intrinsics (`llvm.bswap/ctpop/ctlz/cttz.iN`), with `ctlz`/`cttz`
/// carrying a second `i1 true` is_zero_undef immarg that the interpreter
/// ignores (no poison).
#[test]
fn test_pipeline_intrinsics() {
    let path = c_programs_dir().join("intrinsics.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 2_018_928_754),
        other => panic!("intrinsics.ll: expected I32(2018928754), got {:?}", other),
    }
}

/// Real clang output for signed division/remainder over negative operands
/// (`sdiv` trunc-toward-zero, `srem` sign-of-dividend). 78 = -200 - 20 - 2 + 300.
#[test]
fn test_pipeline_signed() {
    let path = c_programs_dir().join("signed.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 78),
        other => panic!("signed.ll: expected I32(78), got {:?}", other),
    }
}

/// Real clang output for the standard-library memory intrinsics
/// (`llvm.memcpy`/`llvm.memmove`/`llvm.memset`) with stack operands only: the
/// copy/move/set calls carry `ptr align 4` operands and the intrinsic
/// `declare`s carry param attrs. Result 1 = memcpy {1,2,3} to dst, memmove
/// dst[1]=dst[0] (overlap), memset dst[1..3]=0, sum = 1+0+0.
#[test]
fn test_pipeline_memintrin() {
    let path = c_programs_dir().join("memintrin.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 1),
        other => panic!("memintrin.ll: expected I32(1), got {:?}", other),
    }
}

/// Real clang output for exact signed comparisons (Part 2) on negatives and
/// mixed signs: each of `icmp slt/sgt/sle/sge` fires at runtime over helper
/// operands. 59 = the checksum described in signedcmp.c; any predicate that
/// degrades to the unsigned bit-pattern compare flips the answer.
#[test]
fn test_pipeline_signedcmp() {
    let path = c_programs_dir().join("signedcmp.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 59),
        other => panic!("signedcmp.ll: expected I32(59), got {:?}", other),
    }
}

/// Real clang output for i64 (multi-cell) arithmetic: i64 add/sub across the
/// 32-bit limb boundary (borrow clears the high limb, carry re-sets it), two
/// signed i64 compares, a trunc to i32, and i64 parameter passing. 8 =
/// (int)c 6 + (c>0) 1 + (c>2^32) 1.
#[test]
fn test_pipeline_i64arith() {
    let path = c_programs_dir().join("i64arith.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 8),
        other => panic!("i64arith.ll: expected I32(8), got {:?}", other),
    }
}

/// Real clang output for module-level globals: read/modify/write of a mutable
/// global, a pointer relocation (`@greeting -> @.str`), inline `getelementptr`
/// constant expressions into global arrays, a negative i8 scalar, a byte string,
/// and a negative i64 constant. 95 = (41+9) + 41 + s + u + m + h with the four
/// compare flags true. The module contains sub-word/byte data, so it is
/// interpreter-exact only (the VM rejects it; see the driver tests).
#[test]
fn test_pipeline_globals() {
    let path = c_programs_dir().join("globals.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 95),
        other => panic!("globals.ll: expected I32(95), got {:?}", other),
    }
}

/// Real clang output for the full bitwise/shift family: `and`/`or`/`xor`/
/// `shl`/`lshr`/`ashr` at i32, an i8 arithmetic shift through trunc/sext, and
/// two-limb i64 `and`/`xor`/`shl`/`ashr`. The VM backend of these ops is
/// differentially tested against the interpreter in
/// `scratcharch-sair-interpreter` (vm_differential_tests.rs); this fixture
/// proves the translator admits them from real clang IR.
#[test]
fn test_pipeline_bitwise() {
    let path = c_programs_dir().join("bitwise.ll");
    let result = run_llvm_file(path.to_str().unwrap()).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, 293345),
        other => panic!("bitwise.ll: expected I32(293345), got {:?}", other),
    }
}
