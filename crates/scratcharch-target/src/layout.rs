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

use crate::profile::TargetProfile;

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
}
