// Pointer<->integer reinterpretation corpus fixture: real clang -O0 emits
// `ptrtoint`/`inttoptr` for C pointer/integer casts. SA48 pointers are single
// 32-bit cells, so on the VM each form is a zero-cost cell copy; an i64
// `uintptr_t` form must zero-extend (high limb zero) and round-trip.
//
// Every pointer value is checked *through* memory or as an integer — never by
// comparing a raw address across engines — so native, SAIR interpreter, and ISA
// VM can all agree on the returned checksum.
#include <stdint.h>

static uint32_t read_at(uintptr_t addr) {
    return *(const uint32_t *)addr; /* inttoptr(uintptr_t -> ptr) then load */
}

int main(void) {
    int sum = 0;
    uint32_t slot = 300;

    uintptr_t a = (uintptr_t)&slot; /* ptrtoint ptr -> i64 (zero-extended) */
    uintptr_t b = (uintptr_t)((uint32_t *)a); /* inttoptr back, then ptrtoint */
    sum += (a == b) ? 1 : 0;        /* i64 address form survives the round-trip */

    sum += (read_at(a) == 300) ? 2 : 0;  /* i64-carried pointer reads the slot */
    sum += (read_at(b) == 300) ? 4 : 0;  /* ...and the re-cast pointer too */

    /* Byte view of the slot through an integer-carried pointer (LE). */
    unsigned char *cp = (unsigned char *)(uintptr_t)&slot;
    sum += (cp[0] == (unsigned char)(300 & 0xFF)) ? 8 : 0;
    sum += (cp[1] == (unsigned char)(300 >> 8)) ? 16 : 0;

    return 300 + sum;
}
