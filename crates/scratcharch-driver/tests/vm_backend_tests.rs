use std::path::PathBuf;

use scratcharch_driver::{CompileConfig, CompileDriver, ExecutionBackend, ExecutionValue, OptLevel};

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn c_programs_dir() -> PathBuf {
    project_root().join("tests").join("c_programs")
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

fn assert_i32(result: Result<Option<ExecutionValue>, String>, expected: u32, name: &str) {
    match result {
        Ok(Some(ExecutionValue::I32(v))) => assert_eq!(v, expected, "{name}"),
        other => panic!("{name}: expected I32({expected}), got {other:?}"),
    }
}

#[test]
fn test_vm_backend_hello() {
    let path = c_programs_dir().join("hello.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "hello");
}

#[test]
fn test_vm_backend_add() {
    let path = c_programs_dir().join("add.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "add");
}

#[test]
fn test_vm_backend_factorial() {
    let path = c_programs_dir().join("factorial.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 120, "factorial");
}

#[test]
fn test_vm_backend_fib() {
    let path = c_programs_dir().join("fib.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 55, "fib");
}

#[test]
fn test_vm_backend_recursion() {
    let path = c_programs_dir().join("recursion.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 15, "recursion");
}

#[test]
fn test_vm_backend_array() {
    let path = c_programs_dir().join("array.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "array");
}

#[test]
fn test_vm_backend_pointer() {
    let path = c_programs_dir().join("pointer.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::Basic), 42, "pointer");
}

#[test]
fn test_vm_backend_no_opt() {
    let path = c_programs_dir().join("factorial.ll");
    assert_i32(run_llvm_file_vm(path.to_str().unwrap(), OptLevel::None), 120, "factorial no-opt");
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
