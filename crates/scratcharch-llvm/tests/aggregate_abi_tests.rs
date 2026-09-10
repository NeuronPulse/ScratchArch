//! Aggregate ABI lowering (v0.6): the *shape* of what the translator emits, and
//! the layout identities it relies on.
//!
//! An aggregate value is not a SAIR value. It is backed by a compiler-managed
//! temporary slot, moves as a byte-exact copy of its `DataLayout` extent, and
//! crosses a call boundary through the existing positional-cell ABI — a `byval`
//! copy in the callee prologue, a hidden result pointer after the explicit
//! arguments for a returned aggregate. See `docs/design/AGGREGATE_ABI.md`.
//!
//! These tests pin that shape (so a later change cannot quietly turn a copy into
//! an alias, or a fresh slot into a shared one) at the level a reader can check;
//! the end-to-end semantics are covered by the real-clang corpus
//! (`corpus_clang_tests.rs`), the pipeline fixtures, and the three-way
//! native/interpreter/VM differential.

use scratcharch_ir::text;
use scratcharch_llvm::translate_llvm;

/// SAIR text for a module.
fn sair(ir: &str) -> String {
    let module = translate_llvm(ir).expect("IR translates");
    text::serialize(&module)
}

/// The text of the single function named `name`.
fn func_text(module: &str, name: &str) -> String {
    let header = format!("func @{name} ");
    let start = module
        .find(&header)
        .unwrap_or_else(|| panic!("no function {name} in:\n{module}"));
    let rest = &module[start..];
    let end = rest[1..]
        .find("\nfunc @")
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Whether `line` references textual SSA id `id` (so `%1` does not match `%10`).
fn mentions(line: &str, id: usize) -> bool {
    let needle = format!("%{id}");
    line.match_indices(&needle).any(|(i, _)| {
        !line[i + needle.len()..].starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Translate and run `ir`, expecting a single `i32` return value.
fn run_i32(ir: &str) -> u32 {
    let module = translate_llvm(ir).expect("translates");
    let mut interp = scratcharch_sair_interpreter::Interpreter::new(module, 65536, 4096);
    match interp.run().expect("runs") {
        Some(scratcharch_sair_interpreter::RuntimeValue::I32(v)) => v,
        other => panic!("expected I32 result, got {other:?}"),
    }
}

#[test]
fn aggregate_load_copies_the_full_layout_extent() {
    // `struct T { long long v; int k; }` is 16 bytes: 8-byte `v`, 4-byte `k`,
    // then 4 bytes of trailing padding. A `load %T` must copy all 16 bytes —
    // padding included — into a fresh slot, and must not become a scalar read of
    // either live field.
    let text = sair(
        r#"
%T = type { i64, i32 }
define i32 @main() {
entry:
  %p = alloca %T
  %v = load %T, ptr %p
  ret i32 0
}
"#,
    );
    let main = func_text(&text, "main");

    // One slot for `%p` itself, one for the loaded value.
    assert_eq!(main.matches("alloca i8, 16").count(), 2, "{main}");
    assert_eq!(main.matches("load i8 ").count(), 16, "{main}");
    assert_eq!(main.matches("store i8 ").count(), 16, "{main}");
    assert!(
        !main.contains("load i64") && !main.contains("load i32"),
        "an aggregate load must stay a byte copy, got:\n{main}"
    );
}

#[test]
fn aggregate_store_copies_every_byte() {
    // `struct S { int a; int b; int c; }` is 12 bytes with no padding. Both
    // directions are full copies: the `load` fills one fresh slot and the `store`
    // empties it, so 24 byte-loads and 24 byte-stores, and exactly one slot.
    let text = sair(
        r#"
%S = type { i32, i32, i32 }
define void @copy(ptr %dst, ptr %src) {
entry:
  %v = load %S, ptr %src
  store %S %v, ptr %dst
  ret void
}
"#,
    );
    let copy = func_text(&text, "copy");
    assert_eq!(copy.matches("alloca i8, 12").count(), 1, "{copy}");
    assert_eq!(copy.matches("load i8 ").count(), 24, "{copy}");
    assert_eq!(copy.matches("store i8 ").count(), 24, "{copy}");
}

#[test]
fn extractvalue_and_gep_agree_on_the_field_offset() {
    // `struct N { char tag; struct Pair p; int k; }` with `Pair { int a, b; }` is
    // 16 bytes: `tag` at 0, `p` at 4, `k` at 12. A value is written through the
    // *getelementptr* path and read back through *extractvalue* over the identical
    // index path. Both offsets come from `DataLayout`, so the two must agree —
    // for the nested `p.b` leaf (offset 4 + 4 = 8) as well as the flat one.
    let ir = r#"
%Pair = type { i32, i32 }
%N = type { i8, %Pair, i32 }
define i32 @main() {
entry:
  %p = alloca %N
  %t = getelementptr %N, ptr %p, i32 0, i32 0
  store i8 3, ptr %t
  %a = getelementptr %N, ptr %p, i32 0, i32 1, i32 0
  store i32 40, ptr %a
  %b = getelementptr %N, ptr %p, i32 0, i32 1, i32 1
  store i32 400, ptr %b
  %k = getelementptr %N, ptr %p, i32 0, i32 2
  store i32 5000, ptr %k
  %v = load %N, ptr %p
  %rt = extractvalue %N %v, 0
  %ra = extractvalue %N %v, 1, 0
  %rb = extractvalue %N %v, 1, 1
  %rk = extractvalue %N %v, 2
  %s1 = add i32 %ra, %rb
  %s2 = add i32 %s1, %rk
  %z = zext i8 %rt to i32
  %r = add i32 %s2, %z
  ret i32 %r
}
"#;
    let text = sair(ir);

    assert_eq!(
        run_i32(ir),
        5443,
        "3 + 40 + 400 + 5000; module text:\n{text}"
    );
}

#[test]
fn insertvalue_is_a_functional_update() {
    // `insertvalue` yields a *new* aggregate; the operand keeps its old contents.
    // `%w` must therefore be a fresh slot seeded by a full copy of `%v`, not an
    // in-place patch of `%v`'s storage.
    let ir = r#"
%P = type { i32, i32 }
define i32 @main() {
entry:
  %p = alloca %P
  %q = getelementptr %P, ptr %p, i32 0, i32 1
  store i32 11, ptr %p
  store i32 22, ptr %q
  %v = load %P, ptr %p
  %w = insertvalue %P %v, i32 99, 0
  %wa = extractvalue %P %w, 0
  %wb = extractvalue %P %w, 1
  %vc = extractvalue %P %v, 0
  %s = add i32 %wa, %wb
  %r = add i32 %s, %vc
  ret i32 %r
}
"#;
    let text = sair(ir);
    let main = func_text(&text, "main");

    // The `load` fills one slot and the `insertvalue` allocates its own; both
    // exist on top of `%p` itself, so the update is a copy, never a patch.
    assert_eq!(main.matches("alloca i8, 8").count(), 3, "{main}");

    assert_eq!(
        run_i32(ir),
        132,
        "99 + 22 + 11 (the operand is unchanged)"
    );
}

#[test]
fn aggregate_return_appends_a_hidden_result_pointer() {
    // `struct Big` is 20 bytes, so at -O0 clang would use an `sret` pointer. A
    // *register*-returned aggregate (`{ i64, i32 }` for a 12-byte record) has no
    // such pointer in the IR — the translator synthesizes one, appended after the
    // explicit parameters per ABI.md §5.3, and the SAIR function returns void.
    let text = sair(
        r#"
%Triple = type { i32, i32, i32 }
define { i64, i32 } @make(i32 %x) {
entry:
  %p = alloca %Triple
  store i32 %x, ptr %p
  %t = alloca { i64, i32 }
  call void @llvm.memcpy.p0.p0.i64(ptr %t, ptr %p, i64 12, i1 false)
  %v = load { i64, i32 }, ptr %t
  ret { i64, i32 } %v
}
"#,
    );
    let make = func_text(&text, "make");

    assert!(make.contains("func @make -> void"), "{make}");
    // Exactly one explicit parameter, then the appended result pointer.
    assert_eq!(make.matches("  param ").count(), 2, "{make}");
    assert!(make.contains("param i32 %0"), "{make}");
    assert!(make.contains("param ptr %1"), "{make}");
    assert!(
        !make.contains("param i64"),
        "the aggregate is not decomposed into scalar parameters:\n{make}"
    );
}

#[test]
fn byval_is_copied_into_a_private_callee_slot() {
    // A `byval` parameter is a pointer the callee may freely overwrite. The
    // callee must work on its own copy, so the prologue copies the whole object
    // into a fresh slot and rebinds the parameter *name* to that copy: the
    // caller's object survives.
    let ir = r#"
%Big = type { i32, i32, i32, i32, i32 }
define void @clobber(ptr byval(%Big) align 8 %0) {
entry:
  %p = getelementptr %Big, ptr %0, i32 0, i32 0
  store i32 777, ptr %p
  ret void
}
define i32 @main() {
entry:
  %b = alloca %Big
  store i32 1, ptr %b
  call void @clobber(ptr %b)
  %r = load i32, ptr %b
  ret i32 %r
}
"#;
    let text = sair(ir);
    let clobber = func_text(&text, "clobber");

    // The prologue copy: one 20-byte slot filled from the parameter.
    assert_eq!(clobber.matches("alloca i8, 20").count(), 1, "{clobber}");
    assert_eq!(clobber.matches("load i8 ").count(), 20, "{clobber}");
    assert_eq!(clobber.matches("store i8 ").count(), 20, "{clobber}");

    // The parameter is addressed exactly 20 times — every one of them the byte
    // copy's source. The body's field write must go through the copy instead, so
    // a 21st reference to `%0` would mean a write into the caller's object.
    let param_refs = clobber
        .lines()
        .filter(|l| l.contains("gep i8") && mentions(l, 0))
        .count();
    assert_eq!(param_refs, 20, "the byval pointer is used only by the copy:\n{clobber}");

    // Semantically: `clobber` wrote 777 into its own copy, so `main` still reads 1.
    assert_eq!(run_i32(ir), 1, "the callee must not clobber the caller's object");
}

#[test]
fn each_call_site_gets_its_own_result_slot() {
    // Two calls to the same aggregate-returning function must not share result
    // storage: if they did, the second call would overwrite the first result.
    let ir = r#"
%Triple = type { i32, i32, i32 }
define { i64, i32 } @make(i32 %x) {
entry:
  %p = alloca %Triple
  store i32 %x, ptr %p
  %q = getelementptr %Triple, ptr %p, i32 0, i32 1
  %y = add i32 %x, 1
  store i32 %y, ptr %q
  %s = getelementptr %Triple, ptr %p, i32 0, i32 2
  %z = add i32 %x, 2
  store i32 %z, ptr %s
  %t = alloca { i64, i32 }
  call void @llvm.memcpy.p0.p0.i64(ptr %t, ptr %p, i64 12, i1 false)
  %v = load { i64, i32 }, ptr %t
  ret { i64, i32 } %v
}
define i32 @main() {
entry:
  %a = call { i64, i32 } @make(i32 10)
  %b = call { i64, i32 } @make(i32 20)
  %x = extractvalue { i64, i32 } %a, 1
  %y = extractvalue { i64, i32 } %b, 1
  %r = add i32 %x, %y
  ret i32 %r
}
"#;
    let text = sair(ir);
    let main = func_text(&text, "main");

    // Two call sites, two result slots (plus none of their own — the callee
    // allocates its own scratch).
    assert_eq!(main.matches("alloca i8, 16").count(), 2, "{main}");
    // Both calls go to the same void-returning function and pass the slot last.
    assert_eq!(main.matches("call void @make(").count(), 2, "{main}");

    assert_eq!(
        run_i32(ir),
        34,
        "(10 + 2) + (20 + 2); aliased result slots would give 44"
    );
}

#[test]
fn aggregate_arguments_keep_their_positional_order() {
    // Aggregates and scalars interleave in one signature. Each argument keeps its
    // own positional cell, and the explicit arguments precede the synthesized
    // result pointer — never the other way round.
    let text = sair(
        r#"
%Big = type { i32, i32, i32, i32, i32 }
define { i64, i32 } @mix(i32 %s, ptr byval(%Big) align 8 %0) {
entry:
  %p = alloca { i64, i32 }
  store i32 %s, ptr %p
  %v = load { i64, i32 }, ptr %p
  ret { i64, i32 } %v
}
"#,
    );
    let mix = func_text(&text, "mix");

    assert!(mix.contains("func @mix -> void"), "{mix}");
    let params: Vec<&str> = mix
        .lines()
        .filter(|l| l.trim_start().starts_with("param "))
        .collect();
    assert_eq!(
        params,
        vec!["  param i32 %0", "  param ptr %1", "  param ptr %2"],
        "scalar, byval pointer, then the appended result pointer:\n{mix}"
    );
}

#[test]
fn a_named_aggregate_return_type_is_read_as_a_type() {
    // `ret %Pair %v` spells the result type by name. A `%name` is also how an
    // SSA value is written, so the parser must take the *type* reading here —
    // otherwise it consumes `%Pair` as the operand and then trips over the value
    // behind it. The header already accepts the named type, so the body must
    // agree with it.
    let ir = r#"
%Pair = type { i32, i32 }
define %Pair @mk(i32 %x) {
entry:
  %p = alloca %Pair
  store i32 %x, ptr %p
  %q = getelementptr %Pair, ptr %p, i32 0, i32 1
  %y = add i32 %x, 1
  store i32 %y, ptr %q
  %v = load %Pair, ptr %p
  ret %Pair %v
}
define i32 @main() {
entry:
  %n = call %Pair @mk(i32 7)
  %a = extractvalue %Pair %n, 0
  %b = extractvalue %Pair %n, 1
  %r = add i32 %a, %b
  ret i32 %r
}
"#;
    let text = sair(ir);
    // The named aggregate return goes through the same hidden result pointer as
    // a literal one: `mk` returns void and gains a trailing pointer parameter.
    assert!(func_text(&text, "mk").contains("func @mk -> void"), "{text}");
    assert_eq!(run_i32(ir), 15, "7 + 8");
}

#[test]
fn aggregate_abi_lowering_is_deterministic() {
    // The same IR must translate to the same slot layout every time: fresh slots
    // are not assigned from any ambient counter that could vary between runs.
    let ir = r#"
%Triple = type { i32, i32, i32 }
define %Triple @mk(i32 %x) {
entry:
  %p = alloca %Triple
  store i32 %x, ptr %p
  %v = load %Triple, ptr %p
  ret %Triple %v
}
define i32 @main() {
entry:
  %n = call %Triple @mk(i32 5)
  %a = extractvalue %Triple %n, 0
  ret i32 %a
}
"#;
    assert_eq!(sair(ir), sair(ir));
}
