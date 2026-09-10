//! Aggregate data model: the exact static-data image an aggregate global
//! initializer produces.
//!
//! `scratcharch_target::layout` proves the *rules* (padding, nested structs,
//! array-of-struct, struct-of-array, stride, determinism). These tests prove the
//! translation *into bytes*: that an LLVM aggregate initializer is serialized
//! little-endian at the `DataLayout` offsets, that inter-field and trailing
//! padding are exactly zero, that arrays stride by the element's full size, and
//! that a pointer leaf reserves its source stride while storing a 4-byte SAIR
//! address.
//!
//! The image is the module's `static_data.image`, which is indexed relative to
//! [`STATIC_DATA_BASE`], so the assertions below are offsets within it and a
//! stored pointer's value is `STATIC_DATA_BASE + <that global's offset>`.
//!
//! See `docs/design/AGGREGATE_DATA_MODEL.md`.

use scratcharch_ir::module::STATIC_DATA_BASE;
use scratcharch_llvm::translate_llvm;

/// Translate `ir` and return the static-data byte image.
fn image_of(ir: &str) -> Vec<u8> {
    let module = translate_llvm(ir).unwrap_or_else(|e| panic!("translation failed: {e}"));
    module.static_data.image
}

#[test]
fn struct_global_is_serialized_at_data_layout_offsets() {
    // `{ i32, i32 }`: offsets 0 and 4, size 8 — both leaves little-endian.
    let ir = r#"
%T = type { i32, i32 }
@t = global %T { i32 1, i32 2 }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    assert_eq!(image_of(ir), vec![1, 0, 0, 0, 2, 0, 0, 0]);
}

#[test]
fn padded_struct_leaves_interior_and_trailing_padding_zero() {
    // `struct Big { char a; long b; short c; }` — the clang-observable layout:
    // i8@0, i64@8, i16@16, size 24 (7 bytes of interior padding after the char
    // and 6 of trailing padding after the short). The padding must be present in
    // the image and must be zero, because a byte view of the struct observes it.
    let ir = r#"
%B = type { i8, i64, i16 }
@b = global %B { i8 17, i64 5678, i16 9 }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let image = image_of(ir);
    assert_eq!(image.len(), 24, "size must include trailing padding");

    assert_eq!(image[0], 17);
    assert_eq!(image[1..8], [0; 7], "interior padding after the i8");
    assert_eq!(image[8..16], 5678u64.to_le_bytes());
    assert_eq!(image[16..18], 9u16.to_le_bytes());
    assert_eq!(image[18..24], [0; 6], "trailing padding");
}

#[test]
fn nested_aggregate_global_lays_out_recursively() {
    // `[2 x [3 x i32]]`: the outer stride is the inner array's full size (12),
    // and the six leaves land contiguously.
    let ir = r#"
@a = global [2 x [3 x i32]] [[3 x i32] [i32 1, i32 2, i32 3], [3 x i32] [i32 4, i32 5, i32 6]]
define i32 @main() {
entry:
  ret i32 0
}
"#;
    assert_eq!(
        image_of(ir),
        vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 6, 0, 0, 0]
    );
}

#[test]
fn array_of_struct_strides_by_the_full_struct_size() {
    // Each `%P` is 8 bytes, so element 1 starts at 8 — an element stride that is
    // *not* the sum of its field sizes alone once a struct has tail padding.
    let ir = r#"
%P = type { i32, i32 }
@a = global [2 x %P] [%P { i32 1, i32 2 }, %P { i32 3, i32 4 }]
define i32 @main() {
entry:
  ret i32 0
}
"#;
    assert_eq!(image_of(ir), vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
}

#[test]
fn struct_with_an_array_field_places_the_bytes_exactly() {
    // `struct Row { int tag; char name[6]; }` — the array field follows the i32
    // at offset 4 and a `c"…"` string of the declared length fills it exactly.
    // The struct size is 12, not 10: the array's own alignment (1) does not
    // lower the struct's (4, from the i32), so two trailing padding bytes are
    // rounded in — matching `sizeof`.
    let ir = r#"
%R = type { i32, [6 x i8] }
@r = global %R { i32 9, [6 x i8] c"hello\00" }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let image = image_of(ir);
    assert_eq!(image.len(), 12, "size rounds up to the struct alignment");
    assert_eq!(&image[0..4], &9u32.to_le_bytes());
    assert_eq!(&image[4..10], b"hello\0");
    assert_eq!(&image[10..12], [0; 2], "trailing padding");
}

#[test]
fn pointer_field_reserves_the_source_stride_and_stores_a_4_byte_address() {
    // `struct Mixed { int tag; char *name; }` over `char sym[] = "hi"`:
    //
    //   offset 0..3   @sym's bytes, "hi\0"
    //   offset 3..8   padding (nothing follows @sym, so @m aligns to 8)
    //   offset 8..12  m.tag = 8
    //   offset 12..16 padding inside the struct (tag is 4 bytes, ptr aligns to 8)
    //   offset 16..20 the SAIR address of @sym (STATIC_DATA_BASE + 0)
    //   offset 20..24 the unused high half of the 8-byte x86-64 pointer stride
    //
    // The source stride is what struct offsets are computed from; only the low 4
    // bytes are ever read, and the rest stay zero.
    let ir = r#"
%M = type { i32, ptr }
@s = global [3 x i8] c"hi\00"
@m = global %M { i32 8, ptr @s }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let image = image_of(ir);
    assert_eq!(image.len(), 24);

    assert_eq!(&image[0..3], b"hi\0");
    assert_eq!(image[3..8], [0; 5], "padding between @sym and the 8-aligned @m");
    assert_eq!(image[8..12], 8u32.to_le_bytes());
    assert_eq!(image[12..16], [0; 4], "interior padding before the pointer field");
    assert_eq!(
        image[16..20],
        STATIC_DATA_BASE.to_le_bytes(),
        "a pointer field stores the absolute SAIR address of its target"
    );
    assert_eq!(image[20..24], [0; 4], "unused high half of the pointer stride");
}

#[test]
fn zeroinitializer_aggregate_is_the_right_size_and_all_zero() {
    // `zeroinitializer` must still reserve the storage — a later global's offset
    // depends on it — and must leave every byte (including padding) zero.
    let ir = r#"
%P = type { i32, i64 }
@z = global %P zeroinitializer
@after = global i32 7
define i32 @main() {
entry:
  ret i32 0
}
"#;
    let image = image_of(ir);
    // `%P` is 16 bytes (i64 alignment forces 4 bytes of interior padding);
    // `@after` aligns to 4 and lands at 16.
    assert_eq!(image.len(), 20);
    assert_eq!(image[..16], [0; 16]);
    assert_eq!(image[16..20], 7u32.to_le_bytes());
}

#[test]
fn aggregate_layout_is_deterministic_across_translations() {
    // The image is a pure function of (DataLayout, initializer), so translating
    // the same module twice cannot produce different bytes — the property that
    // lets the translator, the interpreter and the ScratchGraph lowerer agree.
    let ir = r#"
%P = type { i8, i64, i16 }
@b = global %P { i8 1, i64 2, i16 3 }
define i32 @main() {
entry:
  ret i32 0
}
"#;
    assert_eq!(image_of(ir), image_of(ir));
}
