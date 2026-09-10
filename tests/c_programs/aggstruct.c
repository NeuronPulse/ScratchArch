/* Aggregate memory (v0.5): struct assignment, nested structs, and field
 * offsets that depend on padding.
 *
 * `q = p` is a whole-struct copy, which clang lowers to `llvm.memcpy` over the
 * struct's byte layout. `struct Big` has interior padding (i8 at offset 0,
 * i64 at 8, i16 at 16, size 24), so the byte offsets only agree with native
 * execution if the translator's layout matches clang's.
 *
 * Expected: 3 + 4 + 5 + 6 + 7 + 1 + 2 + 3 + sizeof(struct Big)(24) + 1 = 56
 */

struct Pair { int x; int y; };
struct Big { char a; long b; short c; };
struct Nest { struct Pair p; int z; };

int main(void) {
  struct Pair p = { 3, 4 };
  struct Pair q;
  q = p;

  struct Nest n = { { 5, 6 }, 7 };
  struct Nest m;
  m = n;

  struct Big g = { 1, 2, 3 };
  char *bp = (char *)&g;

  return q.x + q.y + m.p.x + m.p.y + m.z + g.a + (int)g.b + g.c
       + (int)sizeof(struct Big) + (bp[0] == 1);
}
