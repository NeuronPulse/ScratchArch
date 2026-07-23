struct Pair {
    int x;
    int y;
};

int main() {
    struct Pair p;
    p.x = 10;
    p.y = 20;
    return p.x + p.y;
}
