// Feature: a struct passed by value as a parameter.
//
// Two SysV classes are both exercised here. `struct Pair` is 8 bytes with the
// INTEGER class, so clang coerces it in the IR to a plain `i64` argument. `struct
// Big` is 20 bytes (MEMORY class), so it is passed as `byval(%struct.Big)`: a
// pointer to caller storage that the callee may freely overwrite. The callee must
// therefore work on its own copy — see docs/design/AGGREGATE_ABI.md.
//
// Expected: return 57.
struct Pair {
    int a;
    int b;
};

struct Big {
    int a;
    int b;
    int c;
    int d;
    int e;
};

static int pair_sum(struct Pair p) {
    return p.a * 10 + p.b;
}

static int big_sum(struct Big s) {
    return s.a + s.b + s.c + s.d + s.e;
}

int main(void) {
    struct Pair p = {4, 2};
    struct Big b = {1, 2, 3, 4, 5};
    return pair_sum(p) + big_sum(b); /* 42 + 15 */
}
