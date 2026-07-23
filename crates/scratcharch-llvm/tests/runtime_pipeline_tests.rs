//! Pipeline tests for the ScratchArch Runtime Library (SART).
//!
//! These tests exercise the full LLVM IR -> SAIR -> Runtime -> Interpreter path
//! for runtime intrinsics. Each test defines the required runtime functions with
//! `declare` and relies on the SAIR interpreter's intrinsic dispatcher.

use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::{Interpreter, RuntimeValue};

fn run_llvm(input: &str) -> Result<Option<RuntimeValue>, String> {
    let module = translate_llvm(input).map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp.run().map_err(|e| format!("{:?}", e))
}

fn run_expect_i32(input: &str, expected: u32) {
    let result = run_llvm(input).expect("pipeline failed");
    match result {
        Some(RuntimeValue::I32(v)) => assert_eq!(v, expected),
        other => panic!("expected I32({}), got {:?}", expected, other),
    }
}

#[test]
fn runtime_memset_pipeline() {
    run_expect_i32(
        r#"
declare ptr @__scratcharch_memset(ptr %dest, i32 %c, i32 %n)
declare i32 @__scratcharch_memcmp(ptr %s1, ptr %s2, i32 %n)

define i32 @main() {
entry:
  %expected = alloca [4 x i8]
  %buf = alloca [4 x i8]

  %ep0 = getelementptr [4 x i8], ptr %expected, i32 0, i32 0
  %ep1 = getelementptr [4 x i8], ptr %expected, i32 0, i32 1
  %ep2 = getelementptr [4 x i8], ptr %expected, i32 0, i32 2
  %ep3 = getelementptr [4 x i8], ptr %expected, i32 0, i32 3
  store i8 171, ptr %ep0
  store i8 171, ptr %ep1
  store i8 171, ptr %ep2
  store i8 171, ptr %ep3

  %bp = getelementptr [4 x i8], ptr %buf, i32 0, i32 0
  call ptr @__scratcharch_memset(ptr %bp, i32 171, i32 4)

  %r = call i32 @__scratcharch_memcmp(ptr %bp, ptr %ep0, i32 4)
  ret i32 %r
}
"#,
        0,
    );
}

#[test]
fn runtime_memcpy_pipeline() {
    run_expect_i32(
        r#"
declare ptr @__scratcharch_memcpy(ptr %dest, ptr %src, i32 %n)
declare i32 @__scratcharch_memcmp(ptr %s1, ptr %s2, i32 %n)

define i32 @main() {
entry:
  %src = alloca [4 x i8]
  %dst = alloca [4 x i8]

  %s0 = getelementptr [4 x i8], ptr %src, i32 0, i32 0
  %s1 = getelementptr [4 x i8], ptr %src, i32 0, i32 1
  %s2 = getelementptr [4 x i8], ptr %src, i32 0, i32 2
  %s3 = getelementptr [4 x i8], ptr %src, i32 0, i32 3
  store i8 10, ptr %s0
  store i8 20, ptr %s1
  store i8 30, ptr %s2
  store i8 40, ptr %s3

  %d0 = getelementptr [4 x i8], ptr %dst, i32 0, i32 0
  call ptr @__scratcharch_memcpy(ptr %d0, ptr %s0, i32 4)

  %r = call i32 @__scratcharch_memcmp(ptr %d0, ptr %s0, i32 4)
  ret i32 %r
}
"#,
        0,
    );
}

#[test]
fn runtime_memmove_overlap_backward_pipeline() {
    run_expect_i32(
        r#"
declare ptr @__scratcharch_memmove(ptr %dest, ptr %src, i32 %n)
declare i32 @__scratcharch_memcmp(ptr %s1, ptr %s2, i32 %n)

define i32 @main() {
entry:
  %buf = alloca [8 x i8]
  %expected = alloca [8 x i8]

  %p0 = getelementptr [8 x i8], ptr %buf, i32 0, i32 0
  %p1 = getelementptr [8 x i8], ptr %buf, i32 0, i32 1
  %p2 = getelementptr [8 x i8], ptr %buf, i32 0, i32 2
  %p3 = getelementptr [8 x i8], ptr %buf, i32 0, i32 3
  %p4 = getelementptr [8 x i8], ptr %buf, i32 0, i32 4
  %p5 = getelementptr [8 x i8], ptr %buf, i32 0, i32 5
  %p6 = getelementptr [8 x i8], ptr %buf, i32 0, i32 6
  %p7 = getelementptr [8 x i8], ptr %buf, i32 0, i32 7
  store i8 1, ptr %p0
  store i8 2, ptr %p1
  store i8 3, ptr %p2
  store i8 4, ptr %p3
  store i8 5, ptr %p4
  store i8 6, ptr %p5
  store i8 0, ptr %p6
  store i8 0, ptr %p7

  call ptr @__scratcharch_memmove(ptr %p0, ptr %p2, i32 4)

  %e0 = getelementptr [8 x i8], ptr %expected, i32 0, i32 0
  %e1 = getelementptr [8 x i8], ptr %expected, i32 0, i32 1
  %e2 = getelementptr [8 x i8], ptr %expected, i32 0, i32 2
  %e3 = getelementptr [8 x i8], ptr %expected, i32 0, i32 3
  %e4 = getelementptr [8 x i8], ptr %expected, i32 0, i32 4
  %e5 = getelementptr [8 x i8], ptr %expected, i32 0, i32 5
  %e6 = getelementptr [8 x i8], ptr %expected, i32 0, i32 6
  %e7 = getelementptr [8 x i8], ptr %expected, i32 0, i32 7
  store i8 3, ptr %e0
  store i8 4, ptr %e1
  store i8 5, ptr %e2
  store i8 6, ptr %e3
  store i8 5, ptr %e4
  store i8 6, ptr %e5
  store i8 0, ptr %e6
  store i8 0, ptr %e7

  %r = call i32 @__scratcharch_memcmp(ptr %p0, ptr %e0, i32 8)
  ret i32 %r
}
"#,
        0,
    );
}

#[test]
fn runtime_memcmp_pipeline() {
    run_expect_i32(
        r#"
declare i32 @__scratcharch_memcmp(ptr %s1, ptr %s2, i32 %n)

define i32 @main() {
entry:
  %a = alloca [4 x i8]
  %b = alloca [4 x i8]

  %a0 = getelementptr [4 x i8], ptr %a, i32 0, i32 0
  %a1 = getelementptr [4 x i8], ptr %a, i32 0, i32 1
  %a2 = getelementptr [4 x i8], ptr %a, i32 0, i32 2
  %a3 = getelementptr [4 x i8], ptr %a, i32 0, i32 3
  store i8 1, ptr %a0
  store i8 2, ptr %a1
  store i8 3, ptr %a2
  store i8 4, ptr %a3

  %b0 = getelementptr [4 x i8], ptr %b, i32 0, i32 0
  %b1 = getelementptr [4 x i8], ptr %b, i32 0, i32 1
  %b2 = getelementptr [4 x i8], ptr %b, i32 0, i32 2
  %b3 = getelementptr [4 x i8], ptr %b, i32 0, i32 3
  store i8 1, ptr %b0
  store i8 2, ptr %b1
  store i8 5, ptr %b2
  store i8 4, ptr %b3

  %r = call i32 @__scratcharch_memcmp(ptr %a0, ptr %b0, i32 4)
  ret i32 %r
}
"#,
        0xFFFF_FFFE, // -2 as u32
    );
}

#[test]
fn runtime_strlen_pipeline() {
    run_expect_i32(
        r#"
declare i32 @__scratcharch_strlen(ptr %s)

define i32 @main() {
entry:
  %s = alloca [8 x i8]
  %p0 = getelementptr [8 x i8], ptr %s, i32 0, i32 0
  %p1 = getelementptr [8 x i8], ptr %s, i32 0, i32 1
  %p2 = getelementptr [8 x i8], ptr %s, i32 0, i32 2
  %p3 = getelementptr [8 x i8], ptr %s, i32 0, i32 3
  %p4 = getelementptr [8 x i8], ptr %s, i32 0, i32 4
  %p5 = getelementptr [8 x i8], ptr %s, i32 0, i32 5
  store i8 104, ptr %p0
  store i8 101, ptr %p1
  store i8 108, ptr %p2
  store i8 108, ptr %p3
  store i8 111, ptr %p4
  store i8 0, ptr %p5

  %r = call i32 @__scratcharch_strlen(ptr %p0)
  ret i32 %r
}
"#,
        5,
    );
}

#[test]
fn runtime_strcmp_pipeline() {
    run_expect_i32(
        r#"
declare i32 @__scratcharch_strcmp(ptr %s1, ptr %s2)

define i32 @main() {
entry:
  %a = alloca [8 x i8]
  %b = alloca [8 x i8]

  %a0 = getelementptr [8 x i8], ptr %a, i32 0, i32 0
  %a1 = getelementptr [8 x i8], ptr %a, i32 0, i32 1
  %a2 = getelementptr [8 x i8], ptr %a, i32 0, i32 2
  %a3 = getelementptr [8 x i8], ptr %a, i32 0, i32 3
  store i8 97, ptr %a0
  store i8 98, ptr %a1
  store i8 99, ptr %a2
  store i8 0, ptr %a3

  %b0 = getelementptr [8 x i8], ptr %b, i32 0, i32 0
  %b1 = getelementptr [8 x i8], ptr %b, i32 0, i32 1
  %b2 = getelementptr [8 x i8], ptr %b, i32 0, i32 2
  %b3 = getelementptr [8 x i8], ptr %b, i32 0, i32 3
  store i8 97, ptr %b0
  store i8 98, ptr %b1
  store i8 100, ptr %b2
  store i8 0, ptr %b3

  %r = call i32 @__scratcharch_strcmp(ptr %a0, ptr %b0)
  ret i32 %r
}
"#,
        0xFFFF_FFFF, // -1 as u32
    );
}

#[test]
fn runtime_strcpy_pipeline() {
    run_expect_i32(
        r#"
declare ptr @__scratcharch_strcpy(ptr %dest, ptr %src)
declare i32 @__scratcharch_strcmp(ptr %s1, ptr %s2)

define i32 @main() {
entry:
  %src = alloca [8 x i8]
  %dst = alloca [8 x i8]

  %s0 = getelementptr [8 x i8], ptr %src, i32 0, i32 0
  %s1 = getelementptr [8 x i8], ptr %src, i32 0, i32 1
  %s2 = getelementptr [8 x i8], ptr %src, i32 0, i32 2
  store i8 104, ptr %s0
  store i8 105, ptr %s1
  store i8 0, ptr %s2

  %d0 = getelementptr [8 x i8], ptr %dst, i32 0, i32 0
  call ptr @__scratcharch_strcpy(ptr %d0, ptr %s0)

  %r = call i32 @__scratcharch_strcmp(ptr %d0, ptr %s0)
  ret i32 %r
}
"#,
        0,
    );
}

#[test]
fn runtime_strncpy_pipeline() {
    run_expect_i32(
        r#"
declare ptr @__scratcharch_strncpy(ptr %dest, ptr %src, i32 %n)
declare i32 @__scratcharch_memcmp(ptr %s1, ptr %s2, i32 %n)

define i32 @main() {
entry:
  %src = alloca [8 x i8]
  %dst = alloca [8 x i8]
  %expected = alloca [8 x i8]

  %s0 = getelementptr [8 x i8], ptr %src, i32 0, i32 0
  %s1 = getelementptr [8 x i8], ptr %src, i32 0, i32 1
  %s2 = getelementptr [8 x i8], ptr %src, i32 0, i32 2
  store i8 104, ptr %s0
  store i8 105, ptr %s1
  store i8 0, ptr %s2

  %e0 = getelementptr [8 x i8], ptr %expected, i32 0, i32 0
  %e1 = getelementptr [8 x i8], ptr %expected, i32 0, i32 1
  %e2 = getelementptr [8 x i8], ptr %expected, i32 0, i32 2
  %e3 = getelementptr [8 x i8], ptr %expected, i32 0, i32 3
  %e4 = getelementptr [8 x i8], ptr %expected, i32 0, i32 4
  %e5 = getelementptr [8 x i8], ptr %expected, i32 0, i32 5
  %e6 = getelementptr [8 x i8], ptr %expected, i32 0, i32 6
  %e7 = getelementptr [8 x i8], ptr %expected, i32 0, i32 7
  store i8 104, ptr %e0
  store i8 105, ptr %e1
  store i8 0, ptr %e2
  store i8 0, ptr %e3
  store i8 0, ptr %e4
  store i8 0, ptr %e5
  store i8 0, ptr %e6
  store i8 0, ptr %e7

  %d0 = getelementptr [8 x i8], ptr %dst, i32 0, i32 0
  call ptr @__scratcharch_strncpy(ptr %d0, ptr %s0, i32 8)

  %r = call i32 @__scratcharch_memcmp(ptr %d0, ptr %e0, i32 8)
  ret i32 %r
}
"#,
        0,
    );
}

#[test]
fn runtime_abort_pipeline() {
    let err = run_llvm(
        r#"
declare void @__scratcharch_abort()

define i32 @main() {
entry:
  call void @__scratcharch_abort()
  ret i32 0
}
"#,
    )
    .expect_err("abort should fail the pipeline");
    assert!(
        err.contains("Abort") || err.contains("abort"),
        "expected abort error, got: {}",
        err
    );
}

#[test]
fn runtime_panic_pipeline() {
    let err = run_llvm(
        r#"
declare void @__scratcharch_panic()

define i32 @main() {
entry:
  call void @__scratcharch_panic()
  ret i32 0
}
"#,
    )
    .expect_err("panic should fail the pipeline");
    assert!(
        err.contains("Panic") || err.contains("panic"),
        "expected panic error, got: {}",
        err
    );
}
