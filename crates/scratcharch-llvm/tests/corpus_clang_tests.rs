//! Fresh-clang corpus tests.
//!
//! These tests recompile the committed `tests/c_programs/*.c` fixtures to LLVM
//! IR with clang on *every* run, then push the freshly generated IR through the
//! same LLVM→SAIR→interpreter pipeline the committed fixtures exercise. This
//! guards against the committed `.ll` corpus drifting away from what clang
//! actually produces.
//!
//! clang is required; if it is not on `PATH` the tests skip silently so that
//! `cargo test --workspace` stays green on machines without a C toolchain.
//! `scripts/run_c_tests.sh` drives these tests and fails on a real mismatch.

use std::path::PathBuf;
use std::process::Command;
use std::fs;

use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_sair_interpreter::RuntimeValue;

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/scratcharch-llvm
    p.pop(); // project root
    p
}

fn c_programs_dir() -> PathBuf {
    project_root().join("tests").join("c_programs")
}

fn clang_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

/// Compile `prog.c` to an in-memory `.ll` string. Returns `None` if clang is
/// unavailable.
fn compile_fresh(prog: &str) -> Option<String> {
    if !clang_available() {
        return None;
    }
    let c_file = c_programs_dir().join(format!("{prog}.c"));
    let out_dir = std::env::temp_dir().join("scratcharch_corpus");
    fs::create_dir_all(&out_dir).ok()?;
    let ll_file = out_dir.join(format!("{prog}.ll"));

    let status = Command::new("clang")
        .args(["-S", "-emit-llvm", "-O0", "-Xclang", "-disable-O0-optnone"])
        .arg(&c_file)
        .arg("-o")
        .arg(&ll_file)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    fs::read_to_string(&ll_file).ok()
}

fn run_fresh(prog: &str) -> Result<Option<RuntimeValue>, String> {
    let ir = compile_fresh(prog)
        .ok_or_else(|| format!("cannot compile {prog}.c with clang"))?;
    let module = translate_llvm(&ir).map_err(|e| format!("translate {prog}: {e}"))?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp.run().map_err(|e| format!("run {prog}: {:?}", e))
}

macro_rules! fresh_corpus_test {
    ($name:ident, $prog:literal, $expected:expr) => {
        #[test]
        fn $name() {
            // Skip quietly when clang is unavailable.
            if !clang_available() {
                eprintln!("SKIP {} (clang not found)", $prog);
                return;
            }
            let result = run_fresh($prog).expect(&format!("fresh compile of {}", $prog));
            match result {
                Some(RuntimeValue::I32(v)) => assert_eq!(v, $expected, "fresh {} result", $prog),
                other => panic!("fresh {}: expected I32({}), got {:?}", $prog, $expected, other),
            }
        }
    };
}

fresh_corpus_test!(fresh_hello, "hello", 42);
fresh_corpus_test!(fresh_add, "add", 42);
fresh_corpus_test!(fresh_factorial, "factorial", 120);
fresh_corpus_test!(fresh_fib, "fib", 55);
fresh_corpus_test!(fresh_array, "array", 42);
fresh_corpus_test!(fresh_struct, "struct", 30);
fresh_corpus_test!(fresh_pointer, "pointer", 42);
fresh_corpus_test!(fresh_string, "string", 5);
fresh_corpus_test!(fresh_memory, "memory", 6);
fresh_corpus_test!(fresh_recursion, "recursion", 15);
fresh_corpus_test!(fresh_intrinsics, "intrinsics", 2018928754);
fresh_corpus_test!(fresh_signed, "signed", 78);
fresh_corpus_test!(fresh_memintrin, "memintrin", 1);
fresh_corpus_test!(fresh_signedcmp, "signedcmp", 59);
fresh_corpus_test!(fresh_i64arith, "i64arith", 8);
fresh_corpus_test!(fresh_globals, "globals", 95);
