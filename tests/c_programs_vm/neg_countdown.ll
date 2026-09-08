; Counts down from 3 while n > -3, summing n each step: 3 + 2 + 1 + 0 + -1 + -2 = 3.
; The signed guard `sgt n, -3` on a negative sentinel exercises exact signed
; comparison (via the select expansion) with negative operands on the VM path.
define i32 @main() {
entry:
  br label %loop
loop:
  %n = phi i32 [ 3, %entry ], [ %n1, %body ]
  %sum = phi i32 [ 0, %entry ], [ %s1, %body ]
  %c = icmp sgt i32 %n, -3
  br i1 %c, label %body, label %done
body:
  %n1 = sub i32 %n, 1
  %s1 = add i32 %sum, %n
  br label %loop
done:
  ret i32 %sum
}
