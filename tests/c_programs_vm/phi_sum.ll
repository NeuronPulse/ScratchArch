define i32 @main() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %body ]
  %acc = phi i32 [ 0, %entry ], [ %a1, %body ]
  %c = icmp slt i32 %i, 10
  br i1 %c, label %body, label %done
body:
  %i1 = add i32 %i, 1
  %a1 = add i32 %acc, %i1
  br label %loop
done:
  ret i32 %acc
}
