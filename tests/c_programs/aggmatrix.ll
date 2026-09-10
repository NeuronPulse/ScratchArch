; ModuleID = 'aggmatrix.c'
source_filename = "aggmatrix.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }

@matrix = dso_local global [2 x [3 x i32]] [[3 x i32] [i32 1, i32 2, i32 3], [3 x i32] [i32 4, i32 5, i32 6]], align 16
@pairs = dso_local global [2 x %struct.Pair] [%struct.Pair { i32 7, i32 8 }, %struct.Pair { i32 9, i32 10 }], align 16
@grid = dso_local global [2 x [4 x i8]] [[4 x i8] c"abc\00", [4 x i8] c"de\00\00"], align 1

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca ptr, align 8
  store i32 0, ptr %1, align 4
  store ptr @grid, ptr %2, align 8
  %3 = load i32, ptr @matrix, align 16
  %4 = load i32, ptr getelementptr inbounds ([3 x i32], ptr getelementptr inbounds ([2 x [3 x i32]], ptr @matrix, i64 0, i64 1), i64 0, i64 2), align 4
  %5 = add nsw i32 %3, %4
  %6 = load i32, ptr getelementptr inbounds (%struct.Pair, ptr getelementptr inbounds ([2 x %struct.Pair], ptr @pairs, i64 0, i64 1), i32 0, i32 1), align 4
  %7 = add nsw i32 %5, %6
  %8 = load i8, ptr @grid, align 1
  %9 = sext i8 %8 to i32
  %10 = add nsw i32 %7, %9
  %11 = load i8, ptr getelementptr inbounds ([4 x i8], ptr getelementptr inbounds ([2 x [4 x i8]], ptr @grid, i64 0, i64 1), i64 0, i64 1), align 1
  %12 = sext i8 %11 to i32
  %13 = add nsw i32 %10, %12
  %14 = load ptr, ptr %2, align 8
  %15 = getelementptr inbounds i8, ptr %14, i64 2
  %16 = load i8, ptr %15, align 1
  %17 = sext i8 %16 to i32
  %18 = add nsw i32 %13, %17
  ret i32 %18
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
