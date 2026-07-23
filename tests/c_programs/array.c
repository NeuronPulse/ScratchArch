int main() {
    int arr[5];
    int *ptr = &arr[2];
    *ptr = 42;
    return *ptr;
}
