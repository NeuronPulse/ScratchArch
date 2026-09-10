// Feature: a struct returned by value.
//
// `struct Triple` is 12 bytes, so clang coerces the return to `{ i64, i32 }` and
// returns it in registers; `struct Big` is 20 bytes and is returned through
// caller-allocated storage via a hidden `sret` pointer. Each call must produce its
// own distinct value — the two `make_big` calls below would be corrupted by one
// another if the result storage were shared.
//
// Expected: return 66.
struct Triple {
    int a;
    int b;
    int c;
};

struct Big {
    int a;
    int b;
    int c;
    int d;
    int e;
};

static struct Triple make_triple(int x) {
    struct Triple t;
    t.a = x;
    t.b = x + 1;
    t.c = x + 2;
    return t;
}

static struct Big make_big(int x) {
    struct Big b;
    b.a = x;
    b.b = x + 1;
    b.c = x + 2;
    b.d = x + 3;
    b.e = x + 4;
    return b;
}

int main(void) {
    struct Triple t = make_triple(10); /* 10, 11, 12 */
    struct Big b = make_big(20);       /* 20, 21, 22, 23, 24 */
    struct Big c = make_big(30);       /* 30, 31, 32, 33, 34 */
    return t.c + b.a + c.e;            /* 12 + 20 + 34 */
}
