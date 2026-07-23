//! Portable C-style string routines.

use crate::{ByteMemory, RuntimeError};

/// Compute the length of a null-terminated string, excluding the terminator.
pub fn strlen<M: ByteMemory + ?Sized>(mem: &M, s: u32) -> Result<usize, RuntimeError> {
    let mut len = 0usize;
    loop {
        let addr = s.wrapping_add(len as u32);
        let b = mem.load_u8(addr)?;
        if b == 0 {
            return Ok(len);
        }
        len = len.checked_add(1).ok_or(RuntimeError::UnterminatedString { addr })?;
    }
}

/// Compare two null-terminated strings lexicographically as unsigned bytes.
///
/// Returns a negative value, zero, or a positive value following standard C
/// `strcmp` semantics.
pub fn strcmp<M: ByteMemory + ?Sized>(mem: &M, s1: u32, s2: u32) -> Result<i32, RuntimeError> {
    let mut i = 0u32;
    loop {
        let a = mem.load_u8(s1.wrapping_add(i))?;
        let b = mem.load_u8(s2.wrapping_add(i))?;

        if a != b {
            return Ok(i32::from(a) - i32::from(b));
        }
        if a == 0 {
            return Ok(0);
        }
        i = i.wrapping_add(1);
    }
}

/// Copy a null-terminated string from `src` to `dest`, including the terminator.
/// Returns `dest` on success.
pub fn strcpy<M: ByteMemory + ?Sized>(
    mem: &mut M,
    dest: u32,
    src: u32,
) -> Result<u32, RuntimeError> {
    let mut i = 0u32;
    loop {
        let b = mem.load_u8(src.wrapping_add(i))?;
        mem.store_u8(dest.wrapping_add(i), b)?;
        if b == 0 {
            return Ok(dest);
        }
        i = i.wrapping_add(1);
    }
}

/// Copy up to `n` bytes from `src` to `dest`. If `src` is shorter than `n`, the
/// remainder of `dest` is padded with zeros. The result is always
/// null-terminated when `n > 0` and `src` fits within `n` bytes. Returns `dest`
/// on success.
pub fn strncpy<M: ByteMemory + ?Sized>(
    mem: &mut M,
    dest: u32,
    src: u32,
    n: usize,
) -> Result<u32, RuntimeError> {
    let mut terminated = false;
    for i in 0..n {
        let offset = i as u32;
        let b = if terminated {
            0
        } else {
            let v = mem.load_u8(src.wrapping_add(offset))?;
            if v == 0 {
                terminated = true;
            }
            v
        };
        mem.store_u8(dest.wrapping_add(offset), b)?;
    }

    Ok(dest)
}
