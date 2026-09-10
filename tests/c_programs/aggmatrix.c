/* Aggregate memory (v0.5): global nested arrays and arrays of strings.
 *
 * `int matrix[2][3]` is `[2 x [3 x i32]]` and `char grid[2][4]` is
 * `[2 x [4 x i8]]` whose elements clang prints as `[4 x i8] c"…"` string
 * constants. The outer stride is the *inner array's* full size, so `matrix[1]`
 * must advance 12 bytes (not 3 or 4).
 *
 * Expected: 1 + 6 + 10 + 'a'(97) + 'e'(101) + 'c'(99) = 314
 */

struct Pair { int x; int y; };

int matrix[2][3] = { { 1, 2, 3 }, { 4, 5, 6 } };
struct Pair pairs[2] = { { 7, 8 }, { 9, 10 } };
char grid[2][4] = { "abc", "de" };

int main(void) {
  char *g = grid[0];
  return matrix[0][0] + matrix[1][2] + pairs[1].y + grid[0][0] + grid[1][1] + g[2];
}
