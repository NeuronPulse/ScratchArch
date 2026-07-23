//! Unit tests for the runtime library.

use crate::{
    abort, memcmp, memcpy, memmove, memset, panic, strcmp, strcpy, strlen, strncpy, trap,
    ByteMemory,
};

#[test]
fn memcpy_basic() {
    let mut mem = vec![0u8; 16];
    mem[0..4].copy_from_slice(&[1, 2, 3, 4]);
    let r = memcpy(&mut mem, 8, 0, 4).unwrap();
    assert_eq!(r, 8);
    assert_eq!(&mem[8..12], &[1, 2, 3, 4]);
}

#[test]
fn memcpy_zero_length_is_noop() {
    let mut mem = vec![0u8; 8];
    let r = memcpy(&mut mem, 4, 0, 0).unwrap();
    assert_eq!(r, 4);
    assert!(mem.iter().all(|&b| b == 0));
}

#[test]
fn memmove_non_overlapping() {
    let mut mem = vec![0u8; 16];
    mem[0..4].copy_from_slice(&[10, 20, 30, 40]);
    let r = memmove(&mut mem, 8, 0, 4).unwrap();
    assert_eq!(r, 8);
    assert_eq!(&mem[8..12], &[10, 20, 30, 40]);
}

#[test]
fn memmove_overlap_forward() {
    // dest < src, overlapping (copy region backwards in source).
    let mut mem = vec![0u8; 8];
    mem[0..6].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    memmove(&mut mem, 2, 0, 6).unwrap();
    assert_eq!(&mem[0..8], &[1, 2, 1, 2, 3, 4, 5, 6]);
}

#[test]
fn memmove_overlap_backward() {
    // dest > src, overlapping (copy region forwards in source).
    let mut mem = vec![0u8; 8];
    mem[0..6].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    memmove(&mut mem, 0, 2, 4).unwrap();
    assert_eq!(&mem[0..6], &[3, 4, 5, 6, 5, 6]);
}

#[test]
fn memmove_exact_overlap() {
    let mut mem = vec![1, 2, 3, 4];
    memmove(&mut mem, 0, 0, 4).unwrap();
    assert_eq!(&mem[..], &[1, 2, 3, 4]);
}

#[test]
fn memset_basic() {
    let mut mem = vec![0u8; 8];
    let r = memset(&mut mem, 2, 0xAB, 4).unwrap();
    assert_eq!(r, 2);
    assert_eq!(&mem[0..2], &[0, 0]);
    assert_eq!(&mem[2..6], &[0xAB; 4]);
    assert_eq!(&mem[6..8], &[0, 0]);
}

#[test]
fn memcmp_equal() {
    let mem = vec![1, 2, 3, 4, 1, 2, 3, 4];
    assert_eq!(memcmp(&mem, 0, 4, 4).unwrap(), 0);
}

#[test]
fn memcmp_less() {
    let mem = vec![1, 2, 3, 4, 1, 2, 5, 4];
    assert!(memcmp(&mem, 0, 4, 4).unwrap() < 0);
}

#[test]
fn memcmp_greater() {
    let mem = vec![1, 2, 5, 4, 1, 2, 3, 4];
    assert!(memcmp(&mem, 0, 4, 4).unwrap() > 0);
}

#[test]
fn strlen_basic() {
    let mut mem = vec![0u8; 16];
    let s = b"hello\0";
    mem[0..s.len()].copy_from_slice(s);
    assert_eq!(strlen(&mem, 0).unwrap(), 5);
}

#[test]
fn strlen_empty() {
    let mut mem = vec![0u8; 4];
    mem[0] = 0;
    assert_eq!(strlen(&mem, 0).unwrap(), 0);
}

#[test]
fn strcmp_equal() {
    let mut mem = vec![0u8; 16];
    mem[0..6].copy_from_slice(b"hello\0");
    mem[8..14].copy_from_slice(b"hello\0");
    assert_eq!(strcmp(&mem, 0, 8).unwrap(), 0);
}

#[test]
fn strcmp_different() {
    let mut mem = vec![0u8; 16];
    mem[0..4].copy_from_slice(b"abc\0");
    mem[8..12].copy_from_slice(b"abd\0");
    assert!(strcmp(&mem, 0, 8).unwrap() < 0);
}

#[test]
fn strcmp_prefix() {
    let mut mem = vec![0u8; 16];
    mem[0..3].copy_from_slice(b"ab\0");
    mem[8..12].copy_from_slice(b"abc\0");
    assert!(strcmp(&mem, 0, 8).unwrap() < 0);
}

#[test]
fn strcpy_basic() {
    let mut mem = vec![0u8; 16];
    mem[0..6].copy_from_slice(b"hello\0");
    let r = strcpy(&mut mem, 8, 0).unwrap();
    assert_eq!(r, 8);
    assert_eq!(&mem[8..13], b"hello");
    assert_eq!(mem[13], 0);
}

#[test]
fn strncpy_basic() {
    let mut mem = vec![0xFFu8; 16];
    mem[0..6].copy_from_slice(b"hello\0");
    let r = strncpy(&mut mem, 8, 0, 8).unwrap();
    assert_eq!(r, 8);
    assert_eq!(&mem[8..13], b"hello");
    assert_eq!(mem[13], 0);
    assert_eq!(&mem[14..16], &[0, 0]);
}

#[test]
fn abort_returns_error() {
    assert!(matches!(abort(), Err(crate::RuntimeError::Abort)));
}

#[test]
fn panic_returns_error_with_message() {
    assert!(matches!(
        panic("out of memory"),
        Err(crate::RuntimeError::Panic { msg: Some(_) })
    ));
}

#[test]
fn trap_returns_error() {
    assert!(matches!(trap(), Err(crate::RuntimeError::Trap)));
}

#[test]
fn byte_memory_for_slice() {
    let mut backing = [0u8; 4];
    let mut mem: &mut [u8] = &mut backing;
    mem.store_u8(1, 42).unwrap();
    assert_eq!(mem.load_u8(1).unwrap(), 42);
}
