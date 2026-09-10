/* Aggregate memory (v0.5): global struct initializers.
 *
 * Three shapes clang emits for global aggregate data:
 *   - `{ i8, i64, i16 }` with interior padding (i64 lands at offset 8),
 *   - `[N x i8] c"hi\00"` for a char array global,
 *   - a struct whose field is a *pointer to another global* (`ptr @sym`).
 *
 * The static-data image must carry the layout's padding (zero) and the 4-byte
 * SAIR address of `sym` at the pointer field's 8-byte-aligned offset.
 *
 * Expected: 3 + 4 + 5 + 6 + 7 + 8 + 'h'(104) + sizeof(struct Big)(24) + 1 = 162
 */

struct Pair { int x; int y; };
struct Big { char a; long b; short c; };
struct Mixed { int tag; char *name; };

char sym[] = "hi";

struct Pair one = { 3, 4 };
struct Big big = { 5, 6, 7 };
struct Mixed mixed = { 8, sym };

int main(void) {
  char *bp = (char *)&big;
  return one.x + one.y + big.a + (int)big.b + big.c + mixed.tag
       + (int)mixed.name[0] + (int)sizeof(struct Big) + (bp[0] == 5);
}
