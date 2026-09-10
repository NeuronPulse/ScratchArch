//! Explicit-rejection tests for constructs the LLVM frontend cannot represent.
//!
//! Part 7 (indirect calls / function pointers) and the "unknown variants get an
//! explicit diagnostic" rule: SAIR/ISA `Call`s name their callee statically, so
//! there is no function-pointer ABI. Every such construct must fail with a
//! diagnostic that says *why* — never parse into something wrong or silently
//! miscompile. These tests pin the rejection diagnostics so a later accidental
//! "support" that is actually a silent approximation cannot sneak in unnoticed.

use scratcharch_llvm::translate_llvm;

#[test]
fn indirect_call_via_ssa_value_is_rejected() {
    // `call i32 %fp(i32 1)`: the callee is a local SSA value → indirect call.
    let ir = r#"
define i32 @main() {
entry:
  %fp = alloca ptr
  %x = call i32 %fp(i32 1)
  ret i32 %x
}
"#;
    let err = translate_llvm(ir).expect_err("indirect call must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("indirect call") && msg.contains("no function-pointer ABI"),
        "diagnostic must explain the rejection, got: {msg}"
    );
}

#[test]
fn indirect_call_clang_parameter_form_is_rejected() {
    // Real clang-19 form: a function-pointer parameter is opaque `ptr`; the
    // callee token in `call i32 %fp(i32 %arg)` is an SSA value, not a name.
    let ir = r#"
define i32 @apply(ptr %fp, i32 %arg) {
entry:
  %r = call i32 %fp(i32 %arg)
  ret i32 %r
}
"#;
    let err = translate_llvm(ir).expect_err("indirect call via parameter must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("indirect call"),
        "diagnostic must mention indirect call, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Global-variable unsupported forms: the translator supports scalar /
// zeroinitializer / `c"…"` / null / pointer-relocation / flat-array-of-scalar
// initializers, and (since v0.5) nested aggregate initializers laid out through
// `DataLayout`. What remains must fail with a diagnostic that says why: SAIR has
// no undefined values, a pointer relocation to an undeclared global would
// fabricate an address, and an aggregate *value* has no SAIR representation at
// all. These pin the rejection diagnostics so a later accidental "support" that
// is actually a silent approximation cannot sneak in unnoticed.
//
// The positive side of the v0.5 aggregate model — the exact byte image an
// aggregate initializer produces — lives in `aggregate_tests.rs`.
// ---------------------------------------------------------------------------

#[test]
fn undef_global_initializer_is_rejected() {
    let ir = r#"
@g = global i32 undef
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("undef global initializer must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("undef") && msg.contains("no undefined values"),
        "diagnostic must explain the undef rejection, got: {msg}"
    );
}

#[test]
fn poison_global_initializer_is_rejected() {
    let ir = r#"
@g = global i32 poison
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("poison global initializer must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("poison") && msg.contains("no undefined values"),
        "diagnostic must explain the poison rejection, got: {msg}"
    );
}

#[test]
fn struct_value_type_is_rejected() {
    // A struct *value* has no SAIR representation: there is no aggregate-by-value
    // ABI, so a `load %T` cannot produce an operand. The translator must say so
    // rather than flatten the struct into a scalar.
    let ir = r#"
%T = type { i32, i32 }
define i32 @main() {
entry:
  %p = alloca %T
  %v = load %T, ptr %p
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("struct value type must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("%T") && msg.contains("no SAIR value representation"),
        "diagnostic must explain the struct-value rejection, got: {msg}"
    );
}

#[test]
fn array_value_type_is_rejected() {
    // Same rule for arrays; the diagnostic points at the layout-preserving path.
    let ir = r#"
define i32 @main() {
entry:
  %a = alloca [2 x i32]
  %v = load [2 x i32], ptr %a
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("array value type must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("[2 x i32]") && msg.contains("no SAIR value representation"),
        "diagnostic must explain the array-value rejection, got: {msg}"
    );
}

#[test]
fn zero_sized_array_global_is_rejected() {
    // `[0 x i32]` occupies no storage. Giving it a made-up size would shift every
    // later global's offset, so `DataLayout` rejects it and the translator
    // surfaces that as a diagnostic instead of guessing.
    let ir = r#"
@z = global [0 x i32] zeroinitializer
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("zero-sized global type must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("zero-sized") && msg.contains("[0 x i32]"),
        "diagnostic must explain the zero-sized rejection, got: {msg}"
    );
}

#[test]
fn pointer_relocation_to_undefined_global_is_rejected() {
    // `@missing` is never declared; a relocation to it would fabricate an
    // address, so the translator must reject rather than invent one.
    let ir = r#"
@p = global ptr @missing
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("relocation to an undefined global must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("@missing") && msg.contains("undefined global"),
        "diagnostic must name the missing global, got: {msg}"
    );
}

#[test]
fn function_passed_as_value_is_rejected() {
    // Real clang form of a function-pointer argument: `call i32 @apply(ptr
    // noundef @inc, i32 noundef 41)`. Passing `@inc` (a function) as a value
    // requires taking a function address; the frontend has no such notion, so
    // it must reject rather than fabricate an address.
    let ir = r#"
define i32 @inc(i32 %a) {
entry:
  %r = add i32 %a, 1
  ret i32 %r
}

declare i32 @apply(ptr, i32)

define i32 @main() {
entry:
  %r = call i32 @apply(ptr @inc, i32 41)
  ret i32 %r
}
"#;
    let err = translate_llvm(ir).expect_err("function-as-value must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("@inc") && msg.contains("SSA value"),
        "diagnostic must explain the function-address rejection, got: {msg}"
    );
}
