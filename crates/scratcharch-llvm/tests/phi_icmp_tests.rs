//! v0.2 focused tests: LLVM `phi` → SAIR Phi (diamond CFG, loop-carried
//! phis with back-edge fixups) and exact signed `icmp` on negative operands,
//! executed on the SAIR interpreter.
//!
//! The signed-comparison tests exercise the interpreter reference expansion
//! directly (both sides of the `slt` identity, incl. mixed signs); the driver's
//! VM tests cover the same expansions on the ISA backend.

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

// ── phi: diamond CFG (values defined before the merge) ──────────────

#[test]
fn test_phi_diamond_merge() {
    let result = run_llvm("
define i32 @main() {
entry:
  br label %test
test:
  %c = icmp sgt i32 3, 2
  br i1 %c, label %then, label %else
then:
  br label %merge
else:
  br label %merge
merge:
  %v = phi i32 [ 10, %then ], [ 20, %else ]
  ret i32 %v
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 10);
}

#[test]
fn test_phi_diamond_merge_false_branch() {
    let result = run_llvm("
define i32 @main() {
entry:
  br label %test
test:
  %c = icmp slt i32 1, -5
  br i1 %c, label %then, label %else
then:
  br label %merge
else:
  br label %merge
merge:
  %v = phi i32 [ 10, %then ], [ 20, %else ]
  ret i32 %v
}
").expect("translation/execution failed");
    // 1 <s -5 is false → else → 20.
    assert_eq!(unwrap_i32(result), 20);
}

// ── phi: loop-carried (back-edge operands defined after the header) ──

#[test]
fn test_phi_loop_carrying() {
    // Sum 1..5 = 15. Both loop phis carry values defined in %body, which is
    // textually after the loop header — the forward references exercise the
    // placeholder patch-up path.
    let result = run_llvm("
define i32 @main() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %body ]
  %acc = phi i32 [ 0, %entry ], [ %a1, %body ]
  %c = icmp slt i32 %i, 5
  br i1 %c, label %body, label %done
body:
  %i1 = add i32 %i, 1
  %a1 = add i32 %acc, %i1
  br label %loop
done:
  ret i32 %acc
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 15);
}

// ── phi: nested loops ───────────────────────────────────────────────

#[test]
fn test_phi_nested_loops() {
    // 3 outer iterations × inner sum 1..4 (10) = 30.
    let result = run_llvm("
define i32 @main() {
entry:
  br label %outer
outer:
  %oi = phi i32 [ 0, %entry ], [ %oi1, %idone ]
  %total = phi i32 [ 0, %entry ], [ %t1, %idone ]
  %oc = icmp slt i32 %oi, 3
  br i1 %oc, label %inner, label %odone
inner:
  %ji = phi i32 [ 0, %outer ], [ %ji1, %ibody ]
  %isum = phi i32 [ 0, %outer ], [ %s1, %ibody ]
  %jc = icmp slt i32 %ji, 4
  br i1 %jc, label %ibody, label %idone
ibody:
  %ji1 = add i32 %ji, 1
  %s1 = add i32 %isum, %ji1
  br label %inner
idone:
  %oi1 = add i32 %oi, 1
  %t1 = add i32 %total, %isum
  br label %outer
odone:
  ret i32 %total
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 30);
}

// ── signed icmp, exact on negatives ────────────────────────────────
//
// Each test encodes both sides of a comparison and XORs them into an i32
// result, so a wrong signed result (falling back to unsigned) fails loudly.
// A helper that returns 1 when the predicate holds, else 0.

fn signed_result(src: &str) -> u32 {
    let full = format!("
define i32 @main() {{
entry:
  {src}
}}
");
    unwrap_i32(run_llvm(&full).expect("translation/execution failed"))
}

/// `ret i32 (slt %a, %b ? 1 : 0)` — returns the bit via an i32 select on i1.
fn slt_bit(a: i32, b: i32) -> u32 {
    signed_result(&format!("
  %r = icmp slt i32 {a}, {b}
  %v = select i1 %r, i32 1, i32 0
  ret i32 %v
"))
}

#[test]
fn test_signed_slt_negative_operands() {
    // -5 < -1, but unsigned -5 (0xFFFFFFFB) > unsigned -1 → unsigned compare
    // would give 0; exact signed gives 1.
    assert_eq!(slt_bit(-5, -1), 1);
    assert_eq!(slt_bit(-1, -5), 0);
    assert_eq!(slt_bit(-5, 3), 1);
    assert_eq!(slt_bit(3, -5), 0);
    assert_eq!(slt_bit(-3, 1), 1);
    assert_eq!(slt_bit(0, 1), 1);
    assert_eq!(slt_bit(1, 0), 0);
    assert_eq!(slt_bit(-5, -5), 0);
    assert_eq!(slt_bit(-128, 127), 1); // near INT_MIN/MAX
    assert_eq!(slt_bit(127, -128), 0);
}

// ── icmp across widths ──────────────────────────────────────────────

#[test]
fn test_signed_icmp_i8_i16_i64() {
    // i8: -3 <s -1 → 1.
    let r8 = signed_result("
  %r = icmp slt i8 -3, -1
  %v = select i1 %r, i32 1, i32 0
  ret i32 %v
");
    assert_eq!(r8, 1);

    // i16: -300 <s 200 → 1.
    let r16 = signed_result("
  %r = icmp slt i16 -300, 200
  %v = select i1 %r, i32 1, i32 0
  ret i32 %v
");
    assert_eq!(r16, 1);

    // i64: -(2^63) <s 5 → 1 (i64::MIN). The interpreter's I64 compare reads
    // the full 64-bit cell.
    let r64 = signed_result("
  %r = icmp slt i64 -9223372036854775808, 5
  %v = select i1 %r, i32 1, i32 0
  ret i32 %v
");
    assert_eq!(r64, 1);

    // i64 sle/sge mixed sign, no truncation.
    let r64b = signed_result("
  %a = icmp sge i64 5, -1
  %b = select i1 %a, i32 1, i32 0
  ret i32 %b
");
    assert_eq!(r64b, 1);
}

#[test]
fn test_signed_icmp_orders_negative_flow() {
    // Guarding a loop counter by i32::MIN..: continue while n > -50 (negative
    // sentinel), confirming sgt behaves on negative bounds.
    let result = run_llvm("
define i32 @main() {
entry:
  br label %loop
loop:
  %n = phi i32 [ -49, %entry ], [ %n1, %body ]
  %c = icmp sgt i32 %n, -50
  br i1 %c, label %body, label %done
body:
  %n1 = sub i32 %n, 1
  br label %loop
done:
  ret i32 %n
}
").expect("translation/execution failed");
    // Counts down while n > -50 → stops when n == -50.
    assert_eq!(unwrap_i32(result), 0xFFFF_FFCEu32);
}

#[test]
fn test_phi_switch_pred_remap() {
    // A phi whose incoming predecessor is a switch source block. The switch
    // dispatch chain replaces the source as the target's predecessor, so the
    // phi must list the dispatch block(s) after expansion.
    let result = run_llvm("
define i32 @main() {
entry:
  %x = add i32 1, 0
  switch i32 %x, label %def [
    i32 1, label %case1
  ]
case1:
  br label %merge
def:
  br label %merge
merge:
  %v = phi i32 [ 7, %case1 ], [ 9, %def ]
  ret i32 %v
}
").expect("translation/execution failed");
    assert_eq!(unwrap_i32(result), 7);
}
