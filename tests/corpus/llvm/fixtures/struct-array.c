// Feature: array of structs with a dynamic (loop) index.
// `arr[i].id + arr[i].val` forces a GEP whose struct-field offset is constant
// but whose array index is a runtime phi-carried value — the real shape behind
// array-of-record code. Expected: return 66.
struct Item {
    int id;
    int val;
};

int main(void) {
    struct Item arr[3];
    arr[0].id = 1;
    arr[0].val = 10;
    arr[1].id = 2;
    arr[1].val = 20;
    arr[2].id = 3;
    arr[2].val = 30;
    int s = 0;
    for (int i = 0; i < 3; i++) {
        s += arr[i].id + arr[i].val;
    }
    return s; /* 11 + 22 + 33 */
}
