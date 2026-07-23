define i32 @main() {
entry:
  %p = alloca { i32, i32 }
  %x = getelementptr { i32, i32 }, ptr %p, i32 0, i32 0
  store i32 10, ptr %x
  %y = getelementptr { i32, i32 }, ptr %p, i32 0, i32 1
  store i32 20, ptr %y
  %vx = load i32, ptr %x
  %vy = load i32, ptr %y
  %add = add i32 %vx, %vy
  ret i32 %add
}
