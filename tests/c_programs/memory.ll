declare void @__scratcharch_memcpy(ptr, ptr, i32)

define i32 @main() {
entry:
  %src = alloca [3 x i32]
  %dst = alloca [3 x i32]
  %s0 = getelementptr [3 x i32], ptr %src, i32 0, i32 0
  store i32 1, ptr %s0
  %s1 = getelementptr [3 x i32], ptr %src, i32 0, i32 1
  store i32 2, ptr %s1
  %s2 = getelementptr [3 x i32], ptr %src, i32 0, i32 2
  store i32 3, ptr %s2
  call void @__scratcharch_memcpy(ptr %dst, ptr %src, i32 12)
  %d0 = getelementptr [3 x i32], ptr %dst, i32 0, i32 0
  %v0 = load i32, ptr %d0
  %d1 = getelementptr [3 x i32], ptr %dst, i32 0, i32 1
  %v1 = load i32, ptr %d1
  %d2 = getelementptr [3 x i32], ptr %dst, i32 0, i32 2
  %v2 = load i32, ptr %d2
  %sum1 = add i32 %v0, %v1
  %sum = add i32 %sum1, %v2
  ret i32 %sum
}
