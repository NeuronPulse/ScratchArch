#include <stdint.h>

// llvm.bswap.i16 — byte-swap the low 16 bits.
uint32_t r16(uint16_t x) { return __builtin_bswap16(x); }
// llvm.bswap.i32
uint32_t r32(uint32_t x) { return __builtin_bswap32(x); }
// llvm.ctpop.i32
uint32_t p(uint32_t x) { return __builtin_popcount(x); }
// llvm.ctlz.i32 (second i1 arg: is_zero_undef=false)
uint32_t cl(uint32_t x) { return __builtin_clz(x); }
// llvm.cttz.i32
uint32_t ct(uint32_t x) { return __builtin_ctz(x); }
// llvm.ctlz.i64
uint32_t cl64(uint64_t x) { return __builtin_clzll(x); }
// llvm.cttz.i64
uint32_t ct64(uint64_t x) { return __builtin_ctzll(x); }

int main(void) {
    uint32_t s = 0;
    s += r16(0x1234u);            // bswap16(0x1234) = 0x3412
    s += r32(0x12345678u);        // bswap32(0x12345678) = 0x78563412
    s += p(0xF0F0F0F0u);          // popcount = 16
    s += cl(1u);                  // ctlz(1) = 31
    s += ct(0x80000000u);         // cttz(0x80000000) = 31
    s += cl64(0x8000000000000000ULL); // ctlz = 0
    s += ct64(1ULL);              // cttz = 0
    return (int)s;
}
