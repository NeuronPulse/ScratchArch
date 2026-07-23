define i32 @add(i32 %a, i32 %b) {
entry:
  %r = add i32 %a, %b
  ret i32 %r
}

define i32 @main() {
entry:
  %x = call i32 @add(i32 20, i32 22)
  ret i32 %x
}
