// Feature: i64 member inside a struct (two-limb cells at byte offsets).
// The `long long` member spans two 32-bit cells stored at natural alignment;
// add/sub run per-limb, so a >32-bit constant forces the wide path. Expected: return 45.
struct Big {
    long long v; /* offset 0, two limb cells */
    int k;       /* offset 8 */
};

int main(void) {
    struct Big b;
    b.v = 4294967296LL; /* 2^32: high limb 1, low limb 0 */
    b.k = 3;
    b.v += 42;               /* high limb 1, low limb 42 */
    return (int)(b.v - 4294967296LL) + b.k; /* 42 + 3 */
}
