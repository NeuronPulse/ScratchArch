define i32 @factorial(i32 %n) {
entry:
  %cmp = icmp sgt i32 %n, 1
  br i1 %cmp, label %recurse, label %base

recurse:
  %sub = add i32 %n, -1
  %rec = call i32 @factorial(i32 %sub)
  %mul = mul i32 %n, %rec
  ret i32 %mul

base:
  ret i32 1
}

define i32 @main() {
entry:
  %r = call i32 @factorial(i32 5)
  ret i32 %r
}
