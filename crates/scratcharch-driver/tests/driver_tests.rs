use std::path::PathBuf;

use scratcharch_driver::{CompileConfig, CompileDriver, ExecutionValue, OptLevel};

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn c_programs_dir() -> PathBuf {
    project_root().join("tests").join("c_programs")
}

fn run_llvm_file(path: &str, opt_level: OptLevel) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level,
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
fn test_driver_hello_basic() {
    let path = c_programs_dir().join("hello.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 42, "hello");
}

#[test]
fn test_driver_hello_none() {
    let path = c_programs_dir().join("hello.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::None), 42, "hello");
}

#[test]
fn test_driver_add() {
    let path = c_programs_dir().join("add.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 42, "add");
}

#[test]
fn test_driver_factorial() {
    let path = c_programs_dir().join("factorial.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 120, "factorial");
}

#[test]
fn test_driver_fib() {
    let path = c_programs_dir().join("fib.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 55, "fib");
}

#[test]
fn test_driver_array() {
    let path = c_programs_dir().join("array.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 42, "array");
}

#[test]
fn test_driver_struct() {
    let path = c_programs_dir().join("struct.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 30, "struct");
}

#[test]
fn test_driver_pointer() {
    let path = c_programs_dir().join("pointer.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 42, "pointer");
}

#[test]
fn test_driver_string() {
    let path = c_programs_dir().join("string.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 5, "string");
}

#[test]
fn test_driver_memory() {
    let path = c_programs_dir().join("memory.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 6, "memory");
}

#[test]
fn test_driver_recursion() {
    let path = c_programs_dir().join("recursion.ll");
    assert_i32(run_llvm_file(path.to_str().unwrap(), OptLevel::Basic), 15, "recursion");
}

#[test]
fn test_driver_invalid_llvm_error() {
    let driver = CompileDriver::default();
    let result = driver.compile_and_run("this is not llvm ir");
    assert!(result.is_err(), "invalid LLVM IR should produce a driver error");
}
