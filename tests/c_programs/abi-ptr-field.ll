; ModuleID = 'abi-ptr-field.c'
source_filename = "abi-ptr-field.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.View = type { ptr, i32 }

@g = dso_local global [4 x i32] [i32 5, i32 7, i32 11, i32 13], align 16

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.View, align 8
  store i32 0, ptr %1, align 4
  %3 = call { ptr, i32 } @view_make(ptr noundef @g, i32 noundef 4)
  %4 = getelementptr inbounds { ptr, i32 }, ptr %2, i32 0, i32 0
  %5 = extractvalue { ptr, i32 } %3, 0
  store ptr %5, ptr %4, align 8
  %6 = getelementptr inbounds { ptr, i32 }, ptr %2, i32 0, i32 1
  %7 = extractvalue { ptr, i32 } %3, 1
  store i32 %7, ptr %6, align 8
  %8 = getelementptr inbounds { ptr, i32 }, ptr %2, i32 0, i32 0
  %9 = load ptr, ptr %8, align 8
  %10 = getelementptr inbounds { ptr, i32 }, ptr %2, i32 0, i32 1
  %11 = load i32, ptr %10, align 8
  %12 = call i32 @view_sum(ptr %9, i32 %11)
  ret i32 %12
}

; Function Attrs: noinline nounwind uwtable
define internal { ptr, i32 } @view_make(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca %struct.View, align 8
  %4 = alloca ptr, align 8
  %5 = alloca i32, align 4
  store ptr %0, ptr %4, align 8
  store i32 %1, ptr %5, align 4
  %6 = load ptr, ptr %4, align 8
  %7 = getelementptr inbounds %struct.View, ptr %3, i32 0, i32 0
  store ptr %6, ptr %7, align 8
  %8 = load i32, ptr %5, align 4
  %9 = getelementptr inbounds %struct.View, ptr %3, i32 0, i32 1
  store i32 %8, ptr %9, align 8
  %10 = load { ptr, i32 }, ptr %3, align 8
  ret { ptr, i32 } %10
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @view_sum(ptr %0, i32 %1) #0 {
  %3 = alloca %struct.View, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = getelementptr inbounds { ptr, i32 }, ptr %3, i32 0, i32 0
  store ptr %0, ptr %6, align 8
  %7 = getelementptr inbounds { ptr, i32 }, ptr %3, i32 0, i32 1
  store i32 %1, ptr %7, align 8
  store i32 0, ptr %4, align 4
  store i32 0, ptr %5, align 4
  br label %8

8:                                                ; preds = %22, %2
  %9 = load i32, ptr %5, align 4
  %10 = getelementptr inbounds %struct.View, ptr %3, i32 0, i32 1
  %11 = load i32, ptr %10, align 8
  %12 = icmp slt i32 %9, %11
  br i1 %12, label %13, label %25

13:                                               ; preds = %8
  %14 = getelementptr inbounds %struct.View, ptr %3, i32 0, i32 0
  %15 = load ptr, ptr %14, align 8
  %16 = load i32, ptr %5, align 4
  %17 = sext i32 %16 to i64
  %18 = getelementptr inbounds i32, ptr %15, i64 %17
  %19 = load i32, ptr %18, align 4
  %20 = load i32, ptr %4, align 4
  %21 = add nsw i32 %20, %19
  store i32 %21, ptr %4, align 4
  br label %22

22:                                               ; preds = %13
  %23 = load i32, ptr %5, align 4
  %24 = add nsw i32 %23, 1
  store i32 %24, ptr %5, align 4
  br label %8, !llvm.loop !6

25:                                               ; preds = %8
  %26 = load i32, ptr %4, align 4
  ret i32 %26
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
