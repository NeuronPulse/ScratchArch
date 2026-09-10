/* Aggregate memory (v0.5): a byte view of aggregate memory.
 *
 * Reading an aggregate through an `unsigned char *` observes the exact byte
 * layout: the leading `char` at offset 0, the little-endian bytes of the `long`
 * at offsets 8..16, and the `short` at offsets 16..18 — none of which are
 * reachable through the typed field accesses alone. The array view checks the
 * same bytes from the other direction.
 *
 * Expected (little endian):
 *   b[0]  = 0x11 = 17     b[8]  = 0x99 = 153   b[15] = 0x22 = 34
 *   b[16] = 0x7B = 123    b[17] = 0x7A = 122
 *   a[0]  = 0x11 = 17     a[1]  = 0x22 = 34    a[4]  = 0x05 = 5
 *   a[8]  = 0x09 = 9
 * total = 514
 */

struct Big { char a; long b; short c; };

int arr[3] = { 0x44332211, 0x08070605, 0x0C0B0A09 };
struct Big bg = { 0x11, 0x2233445566778899, 0x7A7B };

int main(void) {
  unsigned char *b = (unsigned char *)&bg;
  unsigned char *a = (unsigned char *)arr;
  return b[0] + b[8] + b[15] + b[16] + b[17]
       + a[0] + a[1] + a[4] + a[8];
}
