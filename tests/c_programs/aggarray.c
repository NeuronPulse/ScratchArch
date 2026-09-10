/* Aggregate memory (v0.5): array of structs and struct containing an array.
 *
 * `pairs[2].y` scales the element pointer by the struct's *full* size (which
 * includes the element's tail padding), and `r.name[1]` adds the offset of an
 * array field inside a struct to the index inside that array — an
 * array -> struct -> array -> scalar chain resolved entirely from the layout.
 *
 * Expected: 1 + 4 + 5 + 9 + 'e'(101) + 6 + sizeof(struct Row)(12) = 138
 */

struct Pair { int x; int y; };
struct Row { int tag; char name[6]; };

int main(void) {
  struct Pair pairs[3];
  pairs[0].x = 1; pairs[0].y = 2;
  pairs[1].x = 3; pairs[1].y = 4;
  pairs[2].x = 5; pairs[2].y = 6;

  struct Row r = { 9, "hello" };

  int m[2][3];
  m[1][2] = pairs[2].y;

  return pairs[0].x + pairs[1].y + pairs[2].x + r.tag + r.name[1] + m[1][2]
       + (int)sizeof(struct Row);
}
