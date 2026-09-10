// Feature: a nested struct returned by value.
//
// `struct Nested` is 12 bytes, so clang coerces the whole aggregate-of-aggregates
// to `{ i64, i32 }` and returns it in registers: no `sret` pointer appears in the
// IR at all. The two calls below must stay independent.
//
// Expected: return 126.
struct Pair {
    int a;
    int b;
};

struct Nested {
    struct Pair p;
    int k;
};

static struct Nested nested_make(int a, int b, int k) {
    struct Nested n;
    n.p.a = a;
    n.p.b = b;
    n.k = k;
    return n;
}

int main(void) {
    struct Nested x = nested_make(1, 2, 3); /* {1, 2}, 3 */
    struct Nested y = nested_make(4, 5, 6); /* {4, 5}, 6 */
    return x.p.a * 100 + x.p.b * 10 + y.k;  /* 100 + 20 + 6 */
}
