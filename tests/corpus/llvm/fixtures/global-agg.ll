; ModuleID = 'global-agg.c'
source_filename = "global-agg.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }

@table = dso_local global [3 x %struct.Pair] [%struct.Pair { i32 1, i32 2 }, %struct.Pair { i32 3, i32 4 }, %struct.Pair { i32 5, i32 6 }], align 16
@base = dso_local global %struct.Pair { i32 10, i32 20 }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4
  store i32 0, ptr %3, align 4
  br label %4

4:                                                ; preds = %21, %0
  %5 = load i32, ptr %3, align 4
  %6 = icmp slt i32 %5, 3
  br i1 %6, label %7, label %24

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 4
  %9 = sext i32 %8 to i64
  %10 = getelementptr inbounds [3 x %struct.Pair], ptr @table, i64 0, i64 %9
  %11 = getelementptr inbounds %struct.Pair, ptr %10, i32 0, i32 0
  %12 = load i32, ptr %11, align 8
  %13 = load i32, ptr %3, align 4
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds [3 x %struct.Pair], ptr @table, i64 0, i64 %14
  %16 = getelementptr inbounds %struct.Pair, ptr %15, i32 0, i32 1
  %17 = load i32, ptr %16, align 4
  %18 = add nsw i32 %12, %17
  %19 = load i32, ptr %2, align 4
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %2, align 4
  br label %21

21:                                               ; preds = %7
  %22 = load i32, ptr %3, align 4
  %23 = add nsw i32 %22, 1
  store i32 %23, ptr %3, align 4
  br label %4, !llvm.loop !6

24:                                               ; preds = %4
  %25 = load i32, ptr @base, align 4
  %26 = load i32, ptr getelementptr inbounds (%struct.Pair, ptr @base, i32 0, i32 1), align 4
  %27 = add nsw i32 %25, %26
  %28 = load i32, ptr %2, align 4
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %2, align 4
  %30 = load i32, ptr %2, align 4
  ret i32 %30
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
!6 = distinct !{!6, !7}
!7 = !{!"llvm.loop.mustprogress"}
