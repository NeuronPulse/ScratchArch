; ModuleID = 'nested-struct.c'
source_filename = "nested-struct.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Rect = type { %struct.Point, %struct.Point, i32 }
%struct.Point = type { i32, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Rect, align 4
  store i32 0, ptr %1, align 4
  %3 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 0
  %4 = getelementptr inbounds %struct.Point, ptr %3, i32 0, i32 0
  store i32 3, ptr %4, align 4
  %5 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 0
  %6 = getelementptr inbounds %struct.Point, ptr %5, i32 0, i32 1
  store i32 4, ptr %6, align 4
  %7 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 1
  %8 = getelementptr inbounds %struct.Point, ptr %7, i32 0, i32 0
  store i32 10, ptr %8, align 4
  %9 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 1
  %10 = getelementptr inbounds %struct.Point, ptr %9, i32 0, i32 1
  store i32 12, ptr %10, align 4
  %11 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 2
  store i32 100, ptr %11, align 4
  %12 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 1
  %13 = getelementptr inbounds %struct.Point, ptr %12, i32 0, i32 0
  %14 = load i32, ptr %13, align 4
  %15 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 0
  %16 = getelementptr inbounds %struct.Point, ptr %15, i32 0, i32 0
  %17 = load i32, ptr %16, align 4
  %18 = sub nsw i32 %14, %17
  %19 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 1
  %20 = getelementptr inbounds %struct.Point, ptr %19, i32 0, i32 1
  %21 = load i32, ptr %20, align 4
  %22 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 0
  %23 = getelementptr inbounds %struct.Point, ptr %22, i32 0, i32 1
  %24 = load i32, ptr %23, align 4
  %25 = sub nsw i32 %21, %24
  %26 = mul nsw i32 %18, %25
  %27 = getelementptr inbounds %struct.Rect, ptr %2, i32 0, i32 2
  %28 = load i32, ptr %27, align 4
  %29 = add nsw i32 %26, %28
  ret i32 %29
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
