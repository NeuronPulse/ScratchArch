//! Data layout abstraction.
//!
//! A `DataLayout` captures the target-dependent facts needed to lay out
//! aggregates and place scalars in memory: pointer size, pointer/integer
//! alignment rules and stack alignment. Aggregate *offsets* are derived here,
//! once, instead of being re-derived ad hoc in each consumer (the SAIR
//! translator, the ScratchGraph backend, later struct lowering).
//!
//! Canonical scalar byte sizes for the SAIR type set (`i1`=1, `i32`=4, `i64`=8,
//! `f64`=8, …) are fixed by the IR type system (`scratcharch_ir::types`) and
//! the core ISA type set (`scratcharch_core::types`); this module's job is the
//! *derived* layout on top of them. For the LLVM frontend the module whose
//! layout governs struct offsets is the source target (x86-64 for the real-IR
//! corpus), so consumers build the layout from the module rather than assuming
//! the 32-bit SA48 pointer.
//!
//! # Aggregates are a layout concern, not a value type
//!
//! [`AggregateType`] describes *memory*, not a SAIR value. SAIR has no
//! aggregate values (there is no aggregate-by-value ABI; see
//! `docs/design/AGGREGATE_DATA_MODEL.md`), so arrays and structs exist on the
//! pipeline only as byte layouts produced here and consumed by the translator,
//! the static-data image, the interpreter and the ScratchGraph backend. That is
//! why this module defines its own [`LayoutScalar`] instead of reusing the IR's
//! `IrType`: `scratcharch-ir` depends on this crate, so the dependency cannot
//! run the other way.
//!
//! # Two sizes per scalar
//!
//! A scalar has a *layout* size and a *stored value width*, and they differ for
//! pointers:
//!
//! * [`DataLayout::scalar_size`] is the source-target stride — 8 bytes for a
//!   pointer in an x86-64-compiled module — and it is what struct field offsets
//!   are computed from, so they agree with clang.
//! * [`LayoutScalar::value_width`] is the number of bytes a memory instruction
//!   actually writes or reads: 4 for a pointer on SA48, 1 for `i1`/`i8`.
//!
//! A pointer leaf therefore reserves its source stride but stores the 32-bit
//! SAIR address in the low bytes; the remaining bytes are padding that is
//! zero-initialized and never interpreted.

use crate::profile::TargetProfile;

/// A scalar type in the memory-layout model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutScalar {
    I1,
    I8,
    I16,
    I32,
    I64,
    F64,
    Ptr,
}

impl LayoutScalar {
    /// Bytes this scalar occupies when written by a memory instruction.
    ///
    /// This is the SAIR/core value width — a SA48 pointer is 32 bits — and is
    /// deliberately *not* [`DataLayout::scalar_size`], which reports the
    /// source-target stride used for struct offsets.
    pub const fn value_width(self) -> u32 {
        match self {
            LayoutScalar::I1 | LayoutScalar::I8 => 1,
            LayoutScalar::I16 => 2,
            LayoutScalar::I32 | LayoutScalar::Ptr => 4,
            LayoutScalar::I64 | LayoutScalar::F64 => 8,
        }
    }

    /// A short name used in diagnostics.
    pub const fn name(self) -> &'static str {
        match self {
            LayoutScalar::I1 => "i1",
            LayoutScalar::I8 => "i8",
            LayoutScalar::I16 => "i16",
            LayoutScalar::I32 => "i32",
            LayoutScalar::I64 => "i64",
            LayoutScalar::F64 => "f64",
            LayoutScalar::Ptr => "ptr",
        }
    }
}

/// A target-aware aggregate memory type: a scalar, an array, or a struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AggregateType {
    Scalar(LayoutScalar),
    /// `[ count x element ]`.
    Array(Box<AggregateType>, u32),
    /// `{ field, … }`, laid out with C natural alignment.
    Struct(Vec<AggregateType>),
}

impl AggregateType {
    pub fn scalar(s: LayoutScalar) -> Self {
        AggregateType::Scalar(s)
    }

    /// A short human-readable form used in diagnostics.
    pub fn describe(&self) -> String {
        match self {
            AggregateType::Scalar(s) => s.name().to_string(),
            AggregateType::Array(inner, n) => format!("[{} x {}]", n, inner.describe()),
            AggregateType::Struct(fields) => {
                let inner: Vec<String> = fields.iter().map(|f| f.describe()).collect();
                format!("{{ {} }}", inner.join(", "))
            }
        }
    }
}

/// The resolved layout of an [`AggregateType`].
///
/// Every type is contiguous with itself, so there is no separate "stride"
/// field: indexing through a pointer to `T` scales by [`TypeLayout::size`], and
/// the distance between consecutive elements of `[N x T]` is the size of `T`
/// (see [`DataLayout::array_stride`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLayout {
    /// Total size in bytes, including trailing padding.
    pub size: u32,
    /// Overall alignment: the max of the component alignments.
    pub align: u32,
    /// Byte offset of each struct field; empty for scalar and array types.
    pub field_offsets: Vec<u32>,
}

impl TypeLayout {
    /// Padding bytes that follow a field occupying `[offset, offset + size)`:
    /// the gap to the next field offset, or to the end of the type.
    ///
    /// Every inter-field and trailing padding byte is therefore derivable from
    /// the layout rather than recomputed by a consumer.
    pub fn padding_after(&self, offset: u32, size: u32) -> u32 {
        let next = self
            .field_offsets
            .iter()
            .copied()
            .filter(|&o| o > offset)
            .min()
            .unwrap_or(self.size);
        next.saturating_sub(offset + size)
    }
}

/// Why a type has no layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// The type occupies no storage (`void`, `{}`, `[0 x T]`).
    ZeroSized { ty: String },
    /// A size computation overflowed `u32`.
    Overflow { ty: String },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::ZeroSized { ty } => {
                write!(f, "cannot lay out zero-sized type {ty}")
            }
            LayoutError::Overflow { ty } => {
                write!(f, "layout of {ty} exceeds the 32-bit address space")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    /// Pointer size in bytes (the number of addressable bytes in a pointer).
    pub pointer_size_bytes: u32,
    /// Natural alignment of a pointer in bytes.
    pub pointer_align_bytes: u32,
}

impl DataLayout {
    /// Layout for an x86-64-style source module (64-bit pointers, 8-byte
    /// pointer alignment). Used when laying out aggregates whose offsets are
    /// defined by clang's `x86_64-pc-linux-gnu` data layout string.
    pub fn x86_64() -> Self {
        DataLayout {
            pointer_size_bytes: 8,
            pointer_align_bytes: 8,
        }
    }

    /// Layout derived from a [`TargetProfile`]. For the SA48 profile this is a
    /// 32-bit-pointer model.
    pub fn from_profile(profile: &TargetProfile) -> Self {
        let bits = profile.pointer_width;
        DataLayout {
            pointer_size_bytes: bits / 8,
            pointer_align_bytes: bits / 8,
        }
    }

    /// Natural alignment of an integer of `bits` bits.
    ///
    /// LLVM uses natural (power-of-two) alignment for the widths in this
    /// frontend's subset: `i1`/`i8` → 1, `i16` → 2, `i32` → 4, `i64` → 8. The
    /// byte count is rounded up to a power of two.
    pub fn integer_align_bytes(bits: u32) -> u32 {
        let bytes = bits.div_ceil(8);
        bytes.next_power_of_two()
    }

    /// Natural alignment of a double (`f64`), which occupies 8 bytes.
    pub const fn f64_align_bytes() -> u32 {
        8
    }

    /// Round `value` up to the next multiple of `align` (power of two).
    pub const fn align_up(value: u32, align: u32) -> u32 {
        if align <= 1 {
            return value;
        }
        (value + align - 1) & !(align - 1)
    }

    /// Source-target size of a scalar, including any pointer stride.
    pub fn scalar_size(&self, s: LayoutScalar) -> u32 {
        match s {
            LayoutScalar::I1 | LayoutScalar::I8 => 1,
            LayoutScalar::I16 => 2,
            LayoutScalar::I32 => 4,
            LayoutScalar::I64 | LayoutScalar::F64 => 8,
            LayoutScalar::Ptr => self.pointer_size_bytes,
        }
    }

    /// Natural alignment of a scalar.
    pub fn scalar_align(&self, s: LayoutScalar) -> u32 {
        match s {
            LayoutScalar::Ptr => self.pointer_align_bytes,
            other => Self::integer_align_bytes(other.value_width() * 8),
        }
    }

    /// Resolve the full layout of `ty`: size, alignment and (for structs) the
    /// byte offset of every field.
    ///
    /// This is the single authority for aggregate geometry; consumers must not
    /// recompute field offsets or element strides themselves.
    pub fn layout_of(&self, ty: &AggregateType) -> Result<TypeLayout, LayoutError> {
        match ty {
            AggregateType::Scalar(s) => Ok(TypeLayout {
                size: self.scalar_size(*s),
                align: self.scalar_align(*s),
                field_offsets: Vec::new(),
            }),
            AggregateType::Array(inner, count) => {
                let element = self.layout_of(inner)?;
                // Elements are contiguous, so an array of `count` elements of
                // the element's own size (which already includes the element's
                // trailing padding) is exactly `size * count`.
                let size = element
                    .size
                    .checked_mul(*count)
                    .ok_or_else(|| LayoutError::Overflow { ty: ty.describe() })?;
                if size == 0 {
                    return Err(LayoutError::ZeroSized { ty: ty.describe() });
                }
                Ok(TypeLayout {
                    size,
                    align: element.align,
                    field_offsets: Vec::new(),
                })
            }
            AggregateType::Struct(fields) => {
                let mut infos = Vec::with_capacity(fields.len());
                for f in fields {
                    let l = self.layout_of(f)?;
                    infos.push((l.size, l.align));
                }
                let laid_out = layout_struct(&infos);
                if laid_out.size == 0 {
                    return Err(LayoutError::ZeroSized { ty: ty.describe() });
                }
                Ok(TypeLayout {
                    size: laid_out.size,
                    align: laid_out.align,
                    field_offsets: laid_out.offsets,
                })
            }
        }
    }

    /// Byte offset of struct field `index`, or `None` if out of bounds.
    pub fn field_offset(&self, ty: &AggregateType, index: usize) -> Option<u32> {
        match ty {
            AggregateType::Struct(_) => self
                .layout_of(ty)
                .ok()
                .and_then(|l| l.field_offsets.get(index).copied()),
            _ => None,
        }
    }

    /// Byte distance between consecutive elements of an array of `element`.
    ///
    /// This is the scale factor the `[N x T]` GEP rule uses, and it is the
    /// element's full size — an array of structs therefore strides by the
    /// struct size *including* its trailing padding, so elements never overlap.
    pub fn array_stride(&self, element: &AggregateType) -> Result<u32, LayoutError> {
        Ok(self.layout_of(element)?.size)
    }
}

/// Result of laying out a struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    /// Total size in bytes including tail padding.
    pub size: u32,
    /// Overall alignment of the struct (max of field alignments).
    pub align: u32,
    /// Byte offset of each field.
    pub offsets: Vec<u32>,
}

/// Lay out a sequence of fields given each field's `(size_bytes, align_bytes)`.
///
/// Offsets follow the C/LLVM natural-alignment rule: each field starts at
/// `align_up(cursor, field_align)`, the struct size is rounded up to the struct
/// alignment (max field alignment), and trailing padding is included.
pub fn layout_struct(fields: &[(u32, u32)]) -> StructLayout {
    let mut cursor: u32 = 0;
    let mut offsets = Vec::with_capacity(fields.len());
    let mut max_align: u32 = 1;
    for (size, align) in fields {
        let align = (*align).max(1);
        max_align = max_align.max(align);
        cursor = DataLayout::align_up(cursor, align);
        offsets.push(cursor);
        cursor = cursor.saturating_add(*size);
    }
    let size = DataLayout::align_up(cursor, max_align);
    StructLayout {
        size,
        align: max_align,
        offsets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x86_64_pointer() {
        let dl = DataLayout::x86_64();
        assert_eq!(dl.pointer_size_bytes, 8);
        assert_eq!(dl.pointer_align_bytes, 8);
    }

    #[test]
    fn test_sa48_from_profile() {
        let dl = DataLayout::from_profile(&TargetProfile::sa48());
        assert_eq!(dl.pointer_size_bytes, 4);
        assert_eq!(dl.pointer_align_bytes, 4);
    }

    #[test]
    fn test_integer_alignment() {
        assert_eq!(DataLayout::integer_align_bytes(1), 1);
        assert_eq!(DataLayout::integer_align_bytes(8), 1);
        assert_eq!(DataLayout::integer_align_bytes(16), 2);
        assert_eq!(DataLayout::integer_align_bytes(32), 4);
        assert_eq!(DataLayout::integer_align_bytes(64), 8);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(DataLayout::align_up(0, 8), 0);
        assert_eq!(DataLayout::align_up(3, 8), 8);
        assert_eq!(DataLayout::align_up(8, 8), 8);
        assert_eq!(DataLayout::align_up(9, 4), 12);
    }

    #[test]
    fn test_struct_two_i32() {
        // struct { i32, i32 } -> offsets [0, 4], size 8.
        let l = layout_struct(&[(4, 4), (4, 4)]);
        assert_eq!(l.offsets, vec![0, 4]);
        assert_eq!(l.size, 8);
        assert_eq!(l.align, 4);
    }

    #[test]
    fn test_struct_mixed_padding() {
        // struct { i8, i32 } -> offsets [0, 4], size 8 (2 bytes padding).
        let l = layout_struct(&[(1, 1), (4, 4)]);
        assert_eq!(l.offsets, vec![0, 4]);
        assert_eq!(l.size, 8);
    }

    #[test]
    fn test_struct_tail_padding() {
        // struct { i64, i8 } -> offsets [0, 8], size 16 (tail padding to i64 align).
        let l = layout_struct(&[(8, 8), (1, 1)]);
        assert_eq!(l.offsets, vec![0, 8]);
        assert_eq!(l.size, 16);
        assert_eq!(l.align, 8);
    }

    #[test]
    fn test_struct_i16_before_i32() {
        // struct { i16, i32, i16 } -> offsets [0, 4, 8], size 12.
        let l = layout_struct(&[(2, 2), (4, 4), (2, 2)]);
        assert_eq!(l.offsets, vec![0, 4, 8]);
        assert_eq!(l.size, 12);
    }

    // -- AggregateType / DataLayout::layout_of -----------------------------

    use AggregateType::{Array, Scalar, Struct};
    use LayoutScalar::{I16, I32, I64, I8, Ptr};

    fn layout(ty: &AggregateType) -> TypeLayout {
        DataLayout::x86_64().layout_of(ty).expect("has a layout")
    }

    #[test]
    fn test_scalar_layout_and_value_width() {
        let dl = DataLayout::x86_64();
        for (s, size, align, width) in [
            (I8, 1, 1, 1),
            (I16, 2, 2, 2),
            (I32, 4, 4, 4),
            (I64, 8, 8, 8),
            (Ptr, 8, 8, 4),
        ] {
            assert_eq!(dl.scalar_size(s), size, "size of {}", s.name());
            assert_eq!(dl.scalar_align(s), align, "align of {}", s.name());
            assert_eq!(s.value_width(), width, "value width of {}", s.name());
        }
    }

    #[test]
    fn test_sa48_scalar_pointer_is_32_bit() {
        let dl = DataLayout::from_profile(&TargetProfile::sa48());
        assert_eq!(dl.scalar_size(Ptr), 4);
        assert_eq!(dl.scalar_align(Ptr), 4);
        // Only the pointer stride is target-dependent; integer layout is not.
        assert_eq!(dl.scalar_size(I64), 8);
    }

    #[test]
    fn test_struct_padding() {
        // struct { i8, i32, i8 }: offsets [0, 4, 8], size 12, align 4.
        let ty = Struct(vec![Scalar(I8), Scalar(I32), Scalar(I8)]);
        let l = layout(&ty);
        assert_eq!(l.field_offsets, vec![0, 4, 8]);
        assert_eq!(l.size, 12);
        assert_eq!(l.align, 4);
        // Three bytes of padding after the leading i8, three after the trailing.
        assert_eq!(l.padding_after(0, 1), 3);
        assert_eq!(l.padding_after(4, 4), 0);
        assert_eq!(l.padding_after(8, 1), 3);
    }

    #[test]
    fn test_nested_struct() {
        // struct { struct { i8, i32 }, i8 } -> inner size 8, outer offsets [0, 8].
        let inner = Struct(vec![Scalar(I8), Scalar(I32)]);
        let outer = Struct(vec![inner, Scalar(I8)]);
        let l = layout(&outer);
        assert_eq!(l.field_offsets, vec![0, 8]);
        assert_eq!(l.size, 12);
        assert_eq!(l.align, 4);
    }

    #[test]
    fn test_arrays() {
        let l = layout(&Array(Box::new(Scalar(I32)), 4));
        assert_eq!(l.size, 16);
        assert_eq!(l.align, 4);
        assert!(l.field_offsets.is_empty());
        assert_eq!(DataLayout::x86_64().array_stride(&Scalar(I32)).unwrap(), 4);
    }

    #[test]
    fn test_nested_arrays() {
        // [2 x [3 x i32]] -> 24 bytes; the outer stride is the inner array size.
        let l = layout(&Array(Box::new(Array(Box::new(Scalar(I32)), 3)), 2));
        assert_eq!(l.size, 24);
        assert_eq!(l.align, 4);
        // The outer stride is the inner array's full size.
        let inner = Array(Box::new(Scalar(I32)), 3);
        assert_eq!(DataLayout::x86_64().array_stride(&inner).unwrap(), 12);
    }

    #[test]
    fn test_array_of_struct() {
        // [3 x struct { i8, i32 }] -> element size 8 (3 bytes padding), total 24.
        let elem = Struct(vec![Scalar(I8), Scalar(I32)]);
        assert_eq!(DataLayout::x86_64().array_stride(&elem).unwrap(), 8);
        let l = layout(&Array(Box::new(elem), 3));
        assert_eq!(l.size, 24);
        assert_eq!(l.align, 4);
    }

    #[test]
    fn test_struct_of_array() {
        // struct { [3 x i8], i32 } -> offsets [0, 4], size 8.
        let ty = Struct(vec![Array(Box::new(Scalar(I8)), 3), Scalar(I32)]);
        let l = layout(&ty);
        assert_eq!(l.field_offsets, vec![0, 4]);
        assert_eq!(l.size, 8);
        assert_eq!(l.align, 4);
    }

    #[test]
    fn test_array_stride_of_struct_includes_tail_padding() {
        // [2 x struct { i8, i64 }] -> element size 16, so the outer size is 32.
        let elem = Struct(vec![Scalar(I8), Scalar(I64)]);
        let dl = DataLayout::x86_64();
        assert_eq!(dl.array_stride(&elem).unwrap(), 16);
        assert_eq!(layout(&Array(Box::new(elem), 2)).size, 32);
    }

    #[test]
    fn test_field_offset_helper() {
        let ty = Struct(vec![Scalar(I8), Scalar(I32)]);
        let dl = DataLayout::x86_64();
        assert_eq!(dl.field_offset(&ty, 0), Some(0));
        assert_eq!(dl.field_offset(&ty, 1), Some(4));
        assert_eq!(dl.field_offset(&ty, 2), None);
        // A non-struct type has no fields.
        assert_eq!(dl.field_offset(&Scalar(I32), 0), None);
    }

    #[test]
    fn test_zero_sized_types_are_rejected() {
        let dl = DataLayout::x86_64();
        assert!(matches!(
            dl.layout_of(&Struct(vec![])),
            Err(LayoutError::ZeroSized { .. })
        ));
        assert!(matches!(
            dl.layout_of(&Array(Box::new(Scalar(I8)), 0)),
            Err(LayoutError::ZeroSized { .. })
        ));
    }

    #[test]
    fn test_zero_sized_component_is_rejected() {
        // A zero-sized component must not silently collapse the whole aggregate
        // to a smaller size.
        let dl = DataLayout::x86_64();
        let empty = Array(Box::new(Scalar(I8)), 0);
        assert!(dl.layout_of(&Struct(vec![Scalar(I32), empty])).is_err());
    }

    #[test]
    fn test_layout_is_deterministic() {
        let ty = Struct(vec![
            Scalar(I16),
            Array(Box::new(Scalar(I8)), 3),
            Struct(vec![Scalar(I64)]),
        ]);
        let dl = DataLayout::x86_64();
        let a = dl.layout_of(&ty).unwrap();
        for _ in 0..8 {
            assert_eq!(dl.layout_of(&ty).unwrap(), a);
        }
    }
}
