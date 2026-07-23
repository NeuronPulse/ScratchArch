int set_and_get(int *p) {
    *p = 42;
    return *p;
}

int main() {
    int x;
    return set_and_get(&x);
}
