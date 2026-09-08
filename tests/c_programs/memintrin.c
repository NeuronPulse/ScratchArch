#include <string.h>

// Exercises llvm.memcpy/memmove/memset as emitted by real clang from the
// standard library memory builtins, with both operands on the stack (no
// global constant sources). Copy 12 bytes of {1,2,3} from src to dst, then
// overwrite the second element in place with memmove from the first byte, and
// finally zero the tail with memset. All three intrinsic families must be
// resolved by the SAIR interpreter.
int main(void) {
  int src[3];
  int dst[3];
  src[0] = 1;
  src[1] = 2;
  src[2] = 3;
  memcpy(dst, src, sizeof dst);      // dst = {1, 2, 3}
  memmove(&dst[1], &dst[0], sizeof(int)); // dst = {1, 1, 3}
  memset(&dst[1], 0, 2 * sizeof(int));    // dst = {1, 0, 0}
  return dst[0] + dst[1] + dst[2];    // 1
}
