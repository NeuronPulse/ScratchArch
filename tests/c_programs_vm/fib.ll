define i32 @fib(i32 %n) {
entry:
  %cmp = icmp sgt i32 %n, 1
  br i1 %cmp, label %recurse, label %base

recurse:
  %sub1 = add i32 %n, -1
  %r1 = call i32 @fib(i32 %sub1)
  %sub2 = add i32 %n, -2
  %r2 = call i32 @fib(i32 %sub2)
  %add = add i32 %r1, %r2
  ret i32 %add

base:
  ret i32 %n
}

define i32 @main() {
entry:
  %r = call i32 @fib(i32 10)
  ret i32 %r
}
