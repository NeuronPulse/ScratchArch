define i32 @main() {
entry:
  %arr = alloca [5 x i32]
  %ptr = getelementptr [5 x i32], ptr %arr, i32 0, i32 2
  store i32 42, ptr %ptr
  %val = load i32, ptr %ptr
  ret i32 %val
}
