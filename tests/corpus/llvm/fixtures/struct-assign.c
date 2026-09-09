// Feature: whole-struct assignment and struct access via a pointer parameter.
// `y = x` copies the record field-by-field (clang -O0, small struct) and a
// pointer-to-struct parameter reads through GEP — the idiomatic C "modify in
// place through a pointer" shape. Expected: return 56.
struct Pair {
    int a;
    int b;
};

static int combine(struct Pair *p) {
    return p->a * 10 + p->b;
}

int main(void) {
    struct Pair x;
    x.a = 4;
    x.b = 2;
    struct Pair y;
    y = x;   /* whole-struct assignment (per-field copy at -O0) */
    y.a = 5;
    return combine(&y) + x.a; /* (5*10+2) + 4 */
}
