use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_sair_interpreter::RuntimeValue;

fn run_llvm(input: &str) -> Result<Option<RuntimeValue>, String> {
    let module = translate_llvm(input).map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp.run().map_err(|e| format!("{:?}", e))
}

fn unwrap_i32(result: Option<RuntimeValue>) -> u32 {
    match result {
        Some(RuntimeValue::I32(v)) => v,
        other => panic!("expected I32, got {:?}", other),
    }
}

// ── Test 1: return constant ──────────────────────────────────────

#[test]
fn test_return_constant() {
    let result = run_llvm("
define i32 @main() {
entry:
  ret i32 42
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

#[test]
fn test_return_constant_void() {
    let result = run_llvm("
define void @main() {
entry:
  ret void
}
").expect("translation/execution failed");
    assert!(result.is_none());
}

// ── Test 2: arithmetic ───────────────────────────────────────────

#[test]
fn test_arithmetic_add() {
    let result = run_llvm("
define i32 @main() {
entry:
  %x = add i32 20, 22
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

#[test]
fn test_arithmetic_mul() {
    let result = run_llvm("
define i32 @main() {
entry:
  %x = mul i32 6, 7
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

#[test]
fn test_arithmetic_sub() {
    let result = run_llvm("
define i32 @main() {
entry:
  %x = sub i32 50, 8
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

// ── Test 3: function call ────────────────────────────────────────

#[test]
fn test_function_call() {
    let result = run_llvm("
define i32 @add(i32 %a, i32 %b) {
entry:
  %r = add i32 %a, %b
  ret i32 %r
}

define i32 @main() {
entry:
  %x = call i32 @add(i32 20, i32 22)
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

// ── Test 4: branch ───────────────────────────────────────────────

#[test]
fn test_branch_true() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cond = icmp eq i32 1, 1
  br i1 %cond, label %then, label %else

then:
  ret i32 1

else:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

#[test]
fn test_branch_false() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cond = icmp eq i32 1, 2
  br i1 %cond, label %then, label %else

then:
  ret i32 1

else:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 0);
}

#[test]
fn test_unconditional_branch() {
    let result = run_llvm("
define i32 @main() {
entry:
  br label %exit

exit:
  ret i32 42
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

// ── Test 5: memory ───────────────────────────────────────────────

#[test]
fn test_alloca_store_load() {
    let result = run_llvm("
define i32 @main() {
entry:
  %ptr = alloca i32
  store i32 42, ptr %ptr
  %val = load i32, ptr %ptr
  ret i32 %val
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

// ── Additional tests: icmp predicates ────────────────────────────

#[test]
fn test_icmp_ne_true() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cond = icmp ne i32 1, 2
  br i1 %cond, label %then, label %else

then:
  ret i32 1

else:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

#[test]
fn test_icmp_ne_false() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cond = icmp ne i32 1, 1
  br i1 %cond, label %then, label %else

then:
  ret i32 1

else:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 0);
}

// ── Error handling tests ─────────────────────────────────────────

#[test]
fn test_reject_unsupported_type() {
    let result = translate_llvm("
define i64 @main() {
entry:
  ret i64 0
}
");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported type"), "error: {err}");
}

// ── C Program Tests ─────────────────────────────────────

#[test]
fn test_c_factorial_5() {
    let result = run_llvm("
define i32 @factorial(i32 %n) {
entry:
  %cmp = icmp sgt i32 %n, 1
  br i1 %cmp, label %recurse, label %base

recurse:
  %sub = add i32 %n, -1
  %rec = call i32 @factorial(i32 %sub)
  %mul = mul i32 %n, %rec
  ret i32 %mul

base:
  ret i32 1
}

define i32 @main() {
entry:
  %r = call i32 @factorial(i32 5)
  ret i32 %r
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 120);
}

#[test]
fn test_c_fib_10() {
    let result = run_llvm("
define i32 @fib(i32 %n) {
entry:
  %cmp = icmp sgt i32 %n, 1
  br i1 %cmp, label %recurse, label %base

recurse:
  %sub1 = add i32 %n, -1
  %r1 = call i32 @fib(i32 %sub1)
  %sub2 = add i32 %n, -2
  %r2 = call i32 @fib(i32 %sub2)
  %add = add i32 %r1, %r2
  ret i32 %add

base:
  ret i32 %n
}

define i32 @main() {
entry:
  %r = call i32 @fib(i32 10)
  ret i32 %r
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 55);
}

#[test]
fn test_c_array_gep() {
    let result = run_llvm("
define i32 @main() {
entry:
  %arr = alloca [5 x i32]
  %ptr = getelementptr [5 x i32], ptr %arr, i32 0, i32 2
  store i32 42, ptr %ptr
  %val = load i32, ptr %ptr
  ret i32 %val
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

#[test]
fn test_c_struct_fields() {
    let result = run_llvm("
define i32 @main() {
entry:
  %p = alloca { i32, i32 }
  %x = getelementptr { i32, i32 }, ptr %p, i32 0, i32 0
  store i32 10, ptr %x
  %y = getelementptr { i32, i32 }, ptr %p, i32 0, i32 1
  store i32 20, ptr %y
  %vx = load i32, ptr %x
  %vy = load i32, ptr %y
  %add = add i32 %vx, %vy
  ret i32 %add
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 30);
}

#[test]
fn test_c_negative_constant() {
    let result = run_llvm("
define i32 @main() {
entry:
  %x = add i32 10, -3
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 7);
}

#[test]
fn test_c_division() {
    let result = run_llvm("
define i32 @main() {
entry:
  %x = sdiv i32 42, 2
  ret i32 %x
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 21);
}

#[test]
fn test_c_icmp_sgt() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cmp = icmp sgt i32 5, 3
  br i1 %cmp, label %t, label %f
t:
  ret i32 1
f:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

#[test]
fn test_c_icmp_sle() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cmp = icmp sle i32 3, 5
  br i1 %cmp, label %t, label %f
t:
  ret i32 1
f:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

#[test]
fn test_c_icmp_sge() {
    let result = run_llvm("
define i32 @main() {
entry:
  %cmp = icmp sge i32 5, 3
  br i1 %cmp, label %t, label %f
t:
  ret i32 1
f:
  ret i32 0
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

#[test]
fn test_reject_unsupported_instruction() {
    let result = translate_llvm("
define i32 @main() {
entry:
  %x = fadd i32 1, 2
  ret i32 %x
}
");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported instruction"), "error: {err}");
}
