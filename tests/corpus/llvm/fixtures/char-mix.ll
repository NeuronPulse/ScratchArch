; ModuleID = 'char-mix.c'
source_filename = "char-mix.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Record = type { i8, i32, i16 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Record, align 4
  store i32 0, ptr %1, align 4
  %3 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 0
  store i8 7, ptr %3, align 4
  %4 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 1
  store i32 1000, ptr %4, align 4
  %5 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 2
  store i16 -12, ptr %5, align 4
  %6 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 0
  %7 = load i8, ptr %6, align 4
  %8 = sext i8 %7 to i32
  %9 = mul nsw i32 %8, 1000
  %10 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 2
  %11 = load i16, ptr %10, align 4
  %12 = sext i16 %11 to i32
  %13 = add nsw i32 %9, %12
  %14 = getelementptr inbounds %struct.Record, ptr %2, i32 0, i32 1
  %15 = load i32, ptr %14, align 4
  %16 = add nsw i32 %13, %15
  ret i32 %16
}

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"Debian clang version 19.1.7 (3+b1)"}
