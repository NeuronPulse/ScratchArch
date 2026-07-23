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
