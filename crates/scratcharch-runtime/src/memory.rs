//! Portable memory routines.

use crate::{ByteMemory, RuntimeError};

/// Copy `n` bytes from `src` to `dest`.
///
/// The source and destination must not overlap; use [`memmove`] for overlapping
/// regions. Returns `dest` on success.
pub fn memcpy<M: ByteMemory + ?Sized>(
    mem: &mut M,
    dest: u32,
    src: u32,
    n: usize,
) -> Result<u32, RuntimeError> {
    for i in 0..n {
        let offset = i as u32;
        let v = mem.load_u8(src.wrapping_add(offset))?;
        mem.store_u8(dest.wrapping_add(offset), v)?;
    }
    Ok(dest)
}

/// Copy `n` bytes from `src` to `dest`, correctly handling overlapping regions.
/// Returns `dest` on success.
pub fn memmove<M: ByteMemory + ?Sized>(
    mem: &mut M,
    dest: u32,
    src: u32,
    n: usize,
) -> Result<u32, RuntimeError> {
    if n == 0 {
        return Ok(dest);
    }

    if dest <= src {
        // Forward copy: safe for non-overlapping and dest-before-src overlaps.
        for i in 0..n {
            let offset = i as u32;
            let v = mem.load_u8(src.wrapping_add(offset))?;
            mem.store_u8(dest.wrapping_add(offset), v)?;
        }
    } else {
        // Backward copy: safe for dest-after-src overlaps.
        for i in (0..n).rev() {
            let offset = i as u32;
            let v = mem.load_u8(src.wrapping_add(offset))?;
            mem.store_u8(dest.wrapping_add(offset), v)?;
        }
    }

    Ok(dest)
}

/// Set `n` bytes starting at `dest` to `c`. Returns `dest` on success.
pub fn memset<M: ByteMemory + ?Sized>(
    mem: &mut M,
    dest: u32,
    c: u8,
    n: usize,
) -> Result<u32, RuntimeError> {
    for i in 0..n {
        mem.store_u8(dest.wrapping_add(i as u32), c)?;
    }
    Ok(dest)
}

/// Compare the first `n` bytes of `s1` and `s2`.
///
/// Returns a negative value if `s1 < s2`, zero if equal, and a positive value
/// if `s1 > s2`. The comparison is unsigned byte-wise, matching standard C
/// semantics.
pub fn memcmp<M: ByteMemory + ?Sized>(
    mem: &M,
    s1: u32,
    s2: u32,
    n: usize,
) -> Result<i32, RuntimeError> {
    for i in 0..n {
        let offset = i as u32;
        let a = mem.load_u8(s1.wrapping_add(offset))?;
        let b = mem.load_u8(s2.wrapping_add(offset))?;
        if a != b {
            return Ok(i32::from(a) - i32::from(b));
        }
    }
    Ok(0)
}
