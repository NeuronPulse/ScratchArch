define i32 @sum(i32 %n) {
entry:
  %zero = icmp eq i32 %n, 0
  br i1 %zero, label %base, label %rec

base:
  ret i32 0

rec:
  %one = sub i32 %n, 1
  %rest = call i32 @sum(i32 %one)
  %total = add i32 %n, %rest
  ret i32 %total
}

define i32 @main() {
entry:
  %r = call i32 @sum(i32 5)
  ret i32 %r
}
