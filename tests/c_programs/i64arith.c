// Exercises i64 (multi-cell) arithmetic through real clang IR: add/sub across
// the 32-bit limb boundary (carry + borrow into/out of the high limb), a signed
// i64 compare, a trunc back to i32, and i64 parameter passing. Values are
// routed through helper calls so clang -O0 cannot fold the i64 ops away.
//
// Chain (all i64, wrapping):
//   a = 4294967296 + 5            = 2^32 + 5   (high limb set)
//   b = a - 6                     = 2^32 - 1   (borrow clears the high limb)
//   c = b + 7                     = 2^32 + 6   (carry re-sets the high limb)
//   pos = (c > 0)  -> signed i64 compare        (1)
//   big = (c > 4294967296) -> signed compare    (1)
// return (int)c + pos + big == 6 + 1 + 1 = 8
static long long sub6(long long x) { return x - 6; }
static long long add7(long long x) { return x + 7; }

int main(void) {
  long long a = 4294967296LL + 5;   // 2^32 + 5
  long long b = sub6(a);            // 2^32 - 1
  long long c = add7(b);            // 2^32 + 6
  int pos = (c > 0) ? 1 : 0;                    // signed i64 sgt
  int big = (c > 4294967296LL) ? 1 : 0;         // signed i64 sgt vs 2^32
  return (int)c + pos + big;
}
