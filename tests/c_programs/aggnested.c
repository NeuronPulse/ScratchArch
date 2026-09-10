/* Aggregate memory (v0.5): a nested aggregate global.
 *
 * `struct Table` holds an array of `struct Line`, each of which holds two
 * `struct Pair`s — so the initializer nests struct-in-array-in-struct four
 * levels deep and the image must be laid out from the `DataLayout` at every
 * level. Reading back through `rows[1].b.y` exercises a
 * array -> struct -> struct -> scalar GEP chain over static data.
 *
 * Expected: 1 + 4 + 5 + 8 + 9 + sizeof(struct Table)(36) = 63
 */

struct Pair { int x; int y; };
struct Line { struct Pair a; struct Pair b; };
struct Table { struct Line rows[2]; int n; };

struct Table t = { { { { 1, 2 }, { 3, 4 } }, { { 5, 6 }, { 7, 8 } } }, 9 };

int main(void) {
  return t.rows[0].a.x + t.rows[0].b.y + t.rows[1].a.x + t.rows[1].b.y + t.n
       + (int)sizeof(struct Table);
}
