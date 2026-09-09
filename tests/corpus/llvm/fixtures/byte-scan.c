// Feature: manual string scan (no libc) over a byte string in static data.
// Walks a `const char[]` with i8 loads, an i8 compare feeding the loop branch,
// and pointer increments — the byte-exact memory/loop core behind hand-rolled
// string code. Expected: return 5.
static const char msg[] = "hello";

int main(void) {
    const char *p = msg;
    int n = 0;
    while (*p != 0) {
        n++;
        p++;
    }
    return n; /* 5 */
}
