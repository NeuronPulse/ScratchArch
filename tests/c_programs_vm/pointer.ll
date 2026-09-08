define i32 @set_and_get(ptr %p) {
entry:
  store i32 42, ptr %p
  %v = load i32, ptr %p
  ret i32 %v
}

define i32 @main() {
entry:
  %x = alloca i32
  %r = call i32 @set_and_get(ptr %x)
  ret i32 %r
}
