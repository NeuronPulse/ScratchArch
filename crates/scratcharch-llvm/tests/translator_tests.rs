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
define double @main() {
entry:
  ret double 0.0
}
");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unsupported type"), "error: {err}");
}

#[test]
fn test_udiv_urem_i64() {
    // SAIR Div/Rem are unsigned floor, so i64 udiv/urem map one-to-one.
    let result = run_llvm("
define i64 @main() {
entry:
  %q = udiv i64 123456789012, 12345
  %r = urem i64 123456789012, 12345
  %x = add i64 %q, %r
  ret i64 %x
}
").expect("translation/execution failed");
    match result {
        Some(RuntimeValue::I64(v)) => assert_eq!(v, 10_012_156),
        other => panic!("expected I64, got {:?}", other),
    }
}

/// `sdiv`/`srem` on negative operands must truncate toward zero and take the
/// dividend's sign. -8/3 = -2 (trunc), -8 % 3 = -2.
#[test]
fn test_sdiv_srem_negative() {
    let result = run_llvm("
define i32 @main() {
entry:
  %q = sdiv i32 -8, 3
  %r = srem i32 -8, 3
  %s = add i32 %q, %r
  ret i32 %s
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 0xFFFF_FFFC); // -2 + -2 = -4
}

#[test]
fn test_sdiv_srem_signs_differ() {
    // 8 / -3 = -2 (trunc); 8 % -3 = +2 (sign of dividend).
    let result = run_llvm("
define i32 @main() {
entry:
  %q = sdiv i32 8, -3
  %r = srem i32 8, -3
  %s = add i32 %q, %r
  ret i32 %s
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 0); // -2 + 2
}

/// INT_MIN / -1 overflows signed division; SAIR's no-poison wrapping policy
/// yields INT_MIN (magnitude division wraps back to itself).
#[test]
fn test_sdiv_i64_min_over_minus_one_wraps() {
    let result = run_llvm("
define i64 @main() {
entry:
  %q = sdiv i64 -9223372036854775808, -1
  ret i64 %q
}
").expect("translation/execution failed");
    match result {
        Some(RuntimeValue::I64(v)) => assert_eq!(v, i64::MIN as u64),
        other => panic!("expected I64, got {:?}", other),
    }
}

// ── i64 support ───────────────────────────────────────────────

#[test]
fn test_i64_constant_return() {
    let result = run_llvm("
define i64 @main() {
entry:
  ret i64 9000000000
}
").expect("translation/execution failed");
    match result {
        Some(RuntimeValue::I64(v)) => assert_eq!(v, 9_000_000_000),
        other => panic!("expected I64, got {:?}", other),
    }
}

#[test]
fn test_i64_arithmetic_wrapping() {
    // 9e9 + 9e9 exceeds 2^34 but not 2^63, so this is exact i64 arithmetic.
    let result = run_llvm("
define i64 @main() {
entry:
  %a = add i64 5000000000, 5000000000
  %b = mul i64 %a, 4
  ret i64 %b
}
").expect("translation/execution failed");
    match result {
        Some(RuntimeValue::I64(v)) => assert_eq!(v, 40_000_000_000),
        other => panic!("expected I64, got {:?}", other),
    }
}

// ── integer conversions ───────────────────────────────────────

#[test]
fn test_zext_i8_to_i32() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = zext i8 255 to i32
  ret i32 %a
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 255);
}

#[test]
fn test_sext_i8_to_i32() {
    // 255 as i8 is -1; sign-extending gives all-ones in i32.
    let result = run_llvm("
define i32 @main() {
entry:
  %a = sext i8 255 to i32
  ret i32 %a
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), u32::MAX);
}

#[test]
fn test_trunc_i64_to_i32() {
    // 0x1_0000_0042 truncated to i32 keeps the low word: 0x42 = 66.
    let result = run_llvm("
define i32 @main() {
entry:
  %a = trunc i64 4294967362 to i32
  ret i32 %a
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 66);
}

#[test]
fn test_ptrtoint_inttoptr_roundtrip() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = alloca i32
  %p = ptrtoint ptr %a to i64
  %q = inttoptr i64 %p to ptr
  store i32 42, ptr %q
  %v = load i32, ptr %q
  ret i32 %v
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 42);
}

// ── control flow ──────────────────────────────────────────────

#[test]
fn test_select_taken() {
    let result = run_llvm("
define i32 @main() {
entry:
  %c = icmp sgt i32 3, 2
  %r = select i1 %c, i32 10, i32 20
  ret i32 %r
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 10);
}

#[test]
fn test_select_not_taken() {
    let result = run_llvm("
define i32 @main() {
entry:
  %c = icmp sgt i32 2, 3
  %r = select i1 %c, i32 10, i32 20
  ret i32 %r
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 20);
}

#[test]
fn test_switch_matching_case() {
    let result = run_llvm("
define i32 @main() {
entry:
  switch i32 2, label %def [
    i32 1, label %one
    i32 2, label %two
    i32 3, label %three
  ]

one:
  ret i32 1
two:
  ret i32 2
three:
  ret i32 3
def:
  ret i32 99
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 2);
}

#[test]
fn test_switch_default() {
    let result = run_llvm("
define i32 @main() {
entry:
  switch i32 7, label %def [
    i32 1, label %one
    i32 2, label %two
  ]

one:
  ret i32 1
two:
  ret i32 2
def:
  ret i32 99
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 99);
}

#[test]
fn test_switch_with_unreachable_default() {
    let result = run_llvm("
define i32 @main() {
entry:
  switch i32 1, label %def [
    i32 1, label %one
  ]

one:
  ret i32 1
def:
  unreachable
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 1);
}

// ── unsigned predicates ───────────────────────────────────────

#[test]
fn test_icmp_unsigned_predicates() {
    let result = run_llvm("
define i32 @main() {
entry:
  %lt = icmp ult i32 10, 5
  %gt = icmp ugt i32 10, 5
  %a = zext i1 %lt to i32
  %b = zext i1 %gt to i32
  %r = add i32 %a, %b
  ret i32 %r
}
").expect("translation/execution failed");
    // 10 < 5 is false (0), 10 > 5 is true (1) -> 1.
    assert_eq!(unwrap_i32(result), 1);
}

// ── layout-based GEP with i64 elements ────────────────────────

#[test]
fn test_gep_i64_array_element() {
    let result = run_llvm("
define i64 @main() {
entry:
  %arr = alloca [4 x i64]
  %p = getelementptr [4 x i64], ptr %arr, i64 0, i64 3
  store i64 77, ptr %p
  %q = getelementptr [4 x i64], ptr %arr, i64 0, i64 3
  %v = load i64, ptr %q
  ret i64 %v
}
").expect("translation/execution failed");
    match result {
        Some(RuntimeValue::I64(v)) => assert_eq!(v, 77),
        other => panic!("expected I64, got {:?}", other),
    }
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

// ── LLVM bit intrinsics (Part 7) ─────────────────────────────────

/// `llvm.bswap.i16`/`i32` — byte-reverse the low bytes. 0x1234 → 0x3412
/// (13330), 0x12345678 → 0x78563412 (2018915346).
#[test]
fn test_llvm_bswap_16_32() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.bswap.i32(i32 305419896)
  %b = call i16 @llvm.bswap.i16(i16 4660)
  %z = zext i16 %b to i32
  %s = add i32 %a, %z
  ret i32 %s
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 2_018_928_676); // 2018915346 + 13330
}

/// `llvm.ctpop.i32` — popcount of 0xF0F0F0F0 is 16.
#[test]
fn test_llvm_ctpop() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.ctpop.i32(i32 -252645136)
  ret i32 %a
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 16);
}

/// `llvm.ctlz`/`llvm.cttz.i32` with the `i1 true` is_zero_undef immarg clang
/// emits for `__builtin_clz/ctz`. ctlz(1)=31, cttz(0x80000000)=31.
#[test]
fn test_llvm_ctlz_cttz_32() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.ctlz.i32(i32 1, i1 true)
  %b = call i32 @llvm.cttz.i32(i32 -2147483648, i1 true)
  %s = add i32 %a, %b
  ret i32 %s
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 62);
}

/// `llvm.ctlz`/`cttz.i64` produce 64-bit results: ctlz(1)=63, cttz(0x8000000000000000)=63.
#[test]
fn test_llvm_ctlz_cttz_64() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i64 @llvm.ctlz.i64(i64 1, i1 true)
  %b = call i64 @llvm.cttz.i64(i64 -9223372036854775808, i1 true)
  %t = trunc i64 %a to i32
  %u = trunc i64 %b to i32
  %s = add i32 %t, %u
  ret i32 %s
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 126);
}

/// Zero operand returns the full width (reference expansion; SAIR has no
/// poison, so even an is_zero_undef=true intrinsic yields the natural value).
#[test]
fn test_llvm_ctlz_zero_returns_width() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.ctlz.i32(i32 0, i1 true)
  ret i32 %a
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 32);
}

/// Unknown `llvm.*` families must fail with an explicit diagnostic, never a
/// silent wrong result.
#[test]
fn test_reject_unknown_llvm_intrinsic() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.fshl.i32(i32 1, i32 1, i32 1)
  ret i32 %a
}
");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unsupported") || msg.contains("llvm.fshl"),
        "expected an explicit unsupported-intrinsic diagnostic, got: {msg}"
    );
}

/// Unsupported bit width in an intrinsic name is reported explicitly.
#[test]
fn test_reject_bad_intrinsic_width() {
    let result = run_llvm("
define i32 @main() {
entry:
  %a = call i32 @llvm.ctpop.i33(i32 7)
  ret i32 %a
}
");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unsupported"), "expected width diagnostic, got: {msg}");
}
