// Feature: a struct with an `i64` member crossing the function boundary.
//
// `struct Wide` is 16 bytes, so clang coerces both the parameter and the return to
// `{ i64, i64 }` — two multi-cell leaves in one aggregate. The wide field's two
// 32-bit cells and the four bytes of trailing padding (offset 12) must survive
// every copy byte-exactly; there is no per-limb ABI and no widening.
//
// Expected: return 7.
struct Wide {
    long long v; /* offset 0, two 32-bit cells */
    int k;       /* offset 8, then 4 bytes of padding */
};

static long long wide_v(struct Wide w) {
    return w.v;
}

static struct Wide wide_make(long long v, int k) {
    struct Wide w;
    w.v = v;
    w.k = k;
    return w;
}

int main(void) {
    struct Wide w = wide_make(4294967300LL, 3); /* 2^32 + 4: high limb 1, low limb 4 */
    long long v = wide_v(w);
    return (int)(v - 4294967296LL) + w.k; /* 4 + 3 */
}
