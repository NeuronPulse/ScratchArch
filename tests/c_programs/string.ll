declare i32 @__scratcharch_strlen(ptr)

define i32 @main() {
entry:
  %s = alloca [6 x i8]
  %p0 = getelementptr [6 x i8], ptr %s, i32 0, i32 0
  store i8 104, ptr %p0
  %p1 = getelementptr [6 x i8], ptr %s, i32 0, i32 1
  store i8 101, ptr %p1
  %p2 = getelementptr [6 x i8], ptr %s, i32 0, i32 2
  store i8 108, ptr %p2
  %p3 = getelementptr [6 x i8], ptr %s, i32 0, i32 3
  store i8 108, ptr %p3
  %p4 = getelementptr [6 x i8], ptr %s, i32 0, i32 4
  store i8 111, ptr %p4
  %p5 = getelementptr [6 x i8], ptr %s, i32 0, i32 5
  store i8 0, ptr %p5
  %len = call i32 @__scratcharch_strlen(ptr %s)
  ret i32 %len
}
