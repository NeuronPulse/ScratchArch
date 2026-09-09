// Feature: struct holding a pointer member + pointer arithmetic through it.
// A struct whose member is `int*` stores a 4-byte SAIR address; dereferencing
// it with a variable index exercises pointer-typed struct fields and dynamic
// GEP. Expected: return 36.
int g[4] = {5, 7, 11, 13};

struct View {
    int *base;
    int len;
};

int main(void) {
    struct View v;
    v.base = g;
    v.len = 4;
    int s = 0;
    for (int i = 0; i < v.len; i++) {
        s += v.base[i];
    }
    return s; /* 5 + 7 + 11 + 13 */
}
