// i64muldiv.c — full-width i64 multiply / divide / remainder through real
// clang IR.
//
// clang -O0 emits `mul` / `udiv` / `urem` / `sdiv` / `srem` over i64 whenever a
// long long (or unsigned long long) is multiplied or divided. Every operation
// below is routed through a static helper that takes its operands as
// parameters, so clang cannot constant-fold the i64 operation away. main()
// folds the low and high 32-bit halves of every result into a checksum — the
// native run returns 3579139508 (0xD55555B4; a shell observing the process
// exit status only sees its low 8 bits, 180) — so a wrong limb — a dropped
// carry, an un-subtracted remainder, a lost sign — perturbs it.
//
// Reference results (wrapping, per SAIR semantics):
//   mmul_u(2^32+1, 2^32+1)  = 2^33 + 1                  (carry across limbs)
//   mmul_u(2^32-1, 2^32-1)  = 2^64 - 2^33 + 1 → hi 2^32-2, lo 1  (all limbs)
//   mmul_s(-10, -10)        = 100                        (signed multiply)
//   udiv64(2^32, 3)         = 1431655765 r 1             (dividend crosses limb)
//   udiv64/urem64(2^63-1, 2^32) = 2147483647 r 4294967295 (full-width operands)
//   sdiv64(-10, 3)          = -3   (trunc toward zero)   srem64(-10, 3) = -1
//   sdiv64(10, -3)          = -3                          srem64(10, -3) = 1
//   sdiv64(INT64_MIN, 2)    = -2^62                       (|INT64_MIN| == 2^63)

static unsigned long long mmul_u(unsigned long long a, unsigned long long b) {
    return a * b;
}
static long long mmul_s(long long a, long long b) {
    return a * b;
}
static unsigned long long udiv64(unsigned long long a, unsigned long long b) {
    return a / b;
}
static unsigned long long urem64(unsigned long long a, unsigned long long b) {
    return a % b;
}
static long long sdiv64(long long a, long long b) {
    return a / b;
}
static long long srem64(long long a, long long b) {
    return a % b;
}

int main(void) {
    // Unsigned multiply with a full-width carry.
    unsigned long long p = mmul_u(4294967297ULL, 4294967297ULL); // (2^32+1)^2
    // Unsigned multiply that engages every 32-bit limb.
    unsigned long long q = mmul_u(4294967295ULL, 4294967295ULL); // (2^32-1)^2
    // Signed multiply.
    long long ms = mmul_s(-10LL, -10LL); // 100

    // Unsigned divide/remainder: dividend crosses the limb boundary.
    unsigned long long dq = udiv64(4294967296ULL, 3ULL);
    unsigned long long dr = urem64(4294967296ULL, 3ULL);
    // Unsigned divide/remainder over full 64-bit operands.
    unsigned long long hd = udiv64(9223372036854775807ULL, 4294967296ULL);
    unsigned long long hr = urem64(9223372036854775807ULL, 4294967296ULL);

    // Signed divide/remainder in each quadrant (trunc toward zero / sign of
    // dividend), plus the exact |INT64_MIN| magnitude edge that does not
    // overflow: INT64_MIN / 2 == -2^62.
    long long sd = sdiv64(-10LL, 3LL);
    long long sr = srem64(-10LL, 3LL);
    long long sd2 = sdiv64(10LL, -3LL);
    long long sr2 = srem64(10LL, -3LL);
    long long neg2 = sdiv64(-9223372036854775807LL - 1LL, 2LL);

    long long acc = 0;
    // Weight the high limb of each result so a dropped limb cannot be masked by
    // a compensating error in the low limb.
    acc += 1000003LL * (long long)(p >> 32) + (long long)p;
    acc += 1000003LL * (long long)(q >> 32) + (long long)q;
    acc += 1000003LL * (long long)(dq >> 32) + (long long)dq;
    acc += 1000003LL * (long long)(hd >> 32) + (long long)hd;
    acc += dr + hr + ms;
    acc += sd + sr + sd2 + sr2 + neg2;
    return (int)acc;
}
