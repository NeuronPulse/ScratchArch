// Feature: global aggregate data (array of structs + one struct) seeded from
// static initializers. Exercises the static-data segment layout for aggregate
// globals with natural alignment, then indexed field reads. Expected: return 51.
struct Pair {
    int a;
    int b;
};

struct Pair table[3] = {{1, 2}, {3, 4}, {5, 6}};
struct Pair base = {10, 20};

int main(void) {
    int s = 0;
    for (int i = 0; i < 3; i++) {
        s += table[i].a + table[i].b;
    }
    s += base.a + base.b;
    return s; /* (3+7+11) + 30 */
}
