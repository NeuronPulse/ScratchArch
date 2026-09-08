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
// Global-variable unsupported forms (Part 6): the translator supports scalar /
// zeroinitializer / `c"…"` / null / pointer-relocation / flat-array-of-scalar
// initializers. Every other form must fail with a diagnostic that says why —
// SAIR has no undefined values, struct literals and nested aggregates have no
// flat serialization, and a pointer relocation to an undeclared global would
// fabricate an address. These pin the rejection diagnostics.
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
fn struct_typed_global_is_rejected() {
    // Struct-typed globals are rejected at the type check, before any
    // initializer is consumed: the flat static-data segment has no struct
    // layout, so a `{ i32 1, i32 2 }` literal could never be serialized.
    let ir = r#"
%T = type { i32, i32 }
@t = global %T { i32 1, i32 2 }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("struct-typed global must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("@t") && msg.contains("struct/void data is not modeled"),
        "diagnostic must explain the struct-global rejection, got: {msg}"
    );
}

#[test]
fn nested_aggregate_global_initializer_is_rejected() {
    let ir = r#"
@a = global [2 x [3 x i32]] [[3 x i32] [i32 1, i32 2, i32 3], [3 x i32] [i32 4, i32 5, i32 6]]
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let err = translate_llvm(ir).expect_err("nested-aggregate global initializer must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("nested-aggregate global initializer"),
        "diagnostic must explain the nested-aggregate rejection, got: {msg}"
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
