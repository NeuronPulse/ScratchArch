; ModuleID = 'string.c'
source_filename = "string.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [6 x i8], align 1
  store i32 0, ptr %1, align 4
  %3 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 0
  store i8 104, ptr %3, align 1
  %4 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 1
  store i8 101, ptr %4, align 1
  %5 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 2
  store i8 108, ptr %5, align 1
  %6 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 3
  store i8 108, ptr %6, align 1
  %7 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 4
  store i8 111, ptr %7, align 1
  %8 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 5
  store i8 0, ptr %8, align 1
  %9 = getelementptr inbounds [6 x i8], ptr %2, i64 0, i64 0
  %10 = call i32 @__scratcharch_strlen(ptr noundef %9)
  ret i32 %10
}

declare i32 @__scratcharch_strlen(ptr noundef) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"Debian clang version 19.1.7 (3+b1)"}
