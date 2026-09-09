// Byte- and sub-word-granular memory corpus fixture: char/short/byte globals
// plus mixed-width loads and stores through real clang -O0 IR.
//
// * `bytes[]`/`pairs[]`/`neg`/`gd[]` are static leaves of i8/i16 width; loading
//   them emits genuine i8/i16 loads with byte-exact LE semantics, and `gd` is
//   mutable so an element write exercises a byte-granular store into static
//   data. (Local brace-array initializers would lower to an `llvm.memcpy`, which
//   is deliberately a separate VM gap, so all arrays here are globals.)
// * `word` reads the four individual bytes of a stored i32 through a `char*`:
//   a byte-granular load into word storage. The fixture assumes little-endian
//   layout (matching both SAIR execution backends), asserting byte order.
// * `bb` builds an i16 out of two byte stores, proving two-byte recombining
//   reads little-endian.
//
// Every check contributes a distinct flag, so a single wrong width, a clobbered
// neighbour, or a wrong byte order flips the returned checksum. Native, SAIR
// interpreter, and ISA VM must all return the same value.
const unsigned char bytes[4] = {0x11, 0x22, 0x33, 0x44};
const unsigned short pairs[2] = {0xBEEF, 0x1234};
const signed char neg = -1;
unsigned char gd[4] = {0xAA, 0xBB, 0xCC, 0xDD};

int main(void) {
    int sum = 0;
    unsigned char a = bytes[0];   /* 0x11 */
    unsigned char b = bytes[3];   /* 0x44 */
    unsigned short w = pairs[1];  /* 0x1234 */
    sum += (int)a + (int)b + (int)(w & 0xFF) + (int)(w >> 8); /* 17+68+52+18 */

    sum += (neg == -1) ? 1 : 0;   /* signed char load compares negative */

    /* An i8 store into static data overwrites exactly one element. */
    gd[1] = 0x5A;
    sum += (gd[0] == 0xAA) ? 1 : 0;
    sum += (gd[1] == 0x5A) ? 2 : 0;
    sum += (gd[2] == 0xCC) ? 4 : 0;
    sum += (gd[3] == 0xDD) ? 8 : 0;

    /* Byte-granular loads of a stored i32 read its little-endian bytes. */
    unsigned int word = 0x78563412u;
    unsigned char *wp = (unsigned char *)&word;
    sum += (wp[0] == 0x12) ? 16 : 0;
    sum += (wp[1] == 0x34) ? 32 : 0;
    sum += (wp[2] == 0x56) ? 64 : 0;
    sum += (wp[3] == 0x78) ? 128 : 0;

    /* Two byte stores reassemble into the expected little-endian i16. */
    unsigned char bb[2];
    bb[0] = 0xEF;
    bb[1] = 0xBE;
    unsigned short z = (unsigned short)(bb[0] | ((unsigned short)bb[1] << 8));
    sum += (z == 0xBEEF) ? 1 : 0;

    return sum;
}
