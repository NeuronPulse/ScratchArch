; ModuleID = 'struct-array.c'
source_filename = "struct-array.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Item = type { i32, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [3 x %struct.Item], align 16
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %5 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 0
  %6 = getelementptr inbounds %struct.Item, ptr %5, i32 0, i32 0
  store i32 1, ptr %6, align 16
  %7 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 0
  %8 = getelementptr inbounds %struct.Item, ptr %7, i32 0, i32 1
  store i32 10, ptr %8, align 4
  %9 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 1
  %10 = getelementptr inbounds %struct.Item, ptr %9, i32 0, i32 0
  store i32 2, ptr %10, align 8
  %11 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 1
  %12 = getelementptr inbounds %struct.Item, ptr %11, i32 0, i32 1
  store i32 20, ptr %12, align 4
  %13 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 2
  %14 = getelementptr inbounds %struct.Item, ptr %13, i32 0, i32 0
  store i32 3, ptr %14, align 16
  %15 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 2
  %16 = getelementptr inbounds %struct.Item, ptr %15, i32 0, i32 1
  store i32 30, ptr %16, align 4
  store i32 0, ptr %3, align 4
  store i32 0, ptr %4, align 4
  br label %17

17:                                               ; preds = %34, %0
  %18 = load i32, ptr %4, align 4
  %19 = icmp slt i32 %18, 3
  br i1 %19, label %20, label %37

20:                                               ; preds = %17
  %21 = load i32, ptr %4, align 4
  %22 = sext i32 %21 to i64
  %23 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 %22
  %24 = getelementptr inbounds %struct.Item, ptr %23, i32 0, i32 0
  %25 = load i32, ptr %24, align 8
  %26 = load i32, ptr %4, align 4
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [3 x %struct.Item], ptr %2, i64 0, i64 %27
  %29 = getelementptr inbounds %struct.Item, ptr %28, i32 0, i32 1
  %30 = load i32, ptr %29, align 4
  %31 = add nsw i32 %25, %30
  %32 = load i32, ptr %3, align 4
  %33 = add nsw i32 %32, %31
  store i32 %33, ptr %3, align 4
  br label %34

34:                                               ; preds = %20
  %35 = load i32, ptr %4, align 4
  %36 = add nsw i32 %35, 1
  store i32 %36, ptr %4, align 4
  br label %17, !llvm.loop !6

37:                                               ; preds = %17
  %38 = load i32, ptr %3, align 4
  ret i32 %38
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
