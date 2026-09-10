// Feature: a struct with a pointer member crossing the function boundary.
//
// `struct View` is 16 bytes, so clang coerces both directions to `{ i64, i64 }`.
// The pointer field is copied as an address — never the pointee — so the callee
// and the caller end up indexing the same global array.
//
// Expected: return 36.
int g[4] = {5, 7, 11, 13};

struct View {
    int *base; /* offset 0 */
    int len;   /* offset 8, then 4 bytes of padding */
};

static int view_sum(struct View v) {
    int s = 0;
    for (int i = 0; i < v.len; i++) {
        s += v.base[i];
    }
    return s;
}

static struct View view_make(int *base, int len) {
    struct View v;
    v.base = base;
    v.len = len;
    return v;
}

int main(void) {
    struct View v = view_make(g, 4);
    return view_sum(v); /* 5 + 7 + 11 + 13 */
}
