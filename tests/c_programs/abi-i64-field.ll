; ModuleID = 'abi-i64-field.c'
source_filename = "abi-i64-field.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Wide = type { i64, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Wide, align 8
  %3 = alloca i64, align 8
  store i32 0, ptr %1, align 4
  %4 = call { i64, i32 } @wide_make(i64 noundef 4294967300, i32 noundef 3)
  %5 = getelementptr inbounds { i64, i32 }, ptr %2, i32 0, i32 0
  %6 = extractvalue { i64, i32 } %4, 0
  store i64 %6, ptr %5, align 8
  %7 = getelementptr inbounds { i64, i32 }, ptr %2, i32 0, i32 1
  %8 = extractvalue { i64, i32 } %4, 1
  store i32 %8, ptr %7, align 8
  %9 = getelementptr inbounds { i64, i32 }, ptr %2, i32 0, i32 0
  %10 = load i64, ptr %9, align 8
  %11 = getelementptr inbounds { i64, i32 }, ptr %2, i32 0, i32 1
  %12 = load i32, ptr %11, align 8
  %13 = call i64 @wide_v(i64 %10, i32 %12)
  store i64 %13, ptr %3, align 8
  %14 = load i64, ptr %3, align 8
  %15 = sub nsw i64 %14, 4294967296
  %16 = trunc i64 %15 to i32
  %17 = getelementptr inbounds %struct.Wide, ptr %2, i32 0, i32 1
  %18 = load i32, ptr %17, align 8
  %19 = add nsw i32 %16, %18
  ret i32 %19
}

; Function Attrs: noinline nounwind uwtable
define internal { i64, i32 } @wide_make(i64 noundef %0, i32 noundef %1) #0 {
  %3 = alloca %struct.Wide, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %0, ptr %4, align 8
  store i32 %1, ptr %5, align 4
  %6 = load i64, ptr %4, align 8
  %7 = getelementptr inbounds %struct.Wide, ptr %3, i32 0, i32 0
  store i64 %6, ptr %7, align 8
  %8 = load i32, ptr %5, align 4
  %9 = getelementptr inbounds %struct.Wide, ptr %3, i32 0, i32 1
  store i32 %8, ptr %9, align 8
  %10 = load { i64, i32 }, ptr %3, align 8
  ret { i64, i32 } %10
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @wide_v(i64 %0, i32 %1) #0 {
  %3 = alloca %struct.Wide, align 8
  %4 = getelementptr inbounds { i64, i32 }, ptr %3, i32 0, i32 0
  store i64 %0, ptr %4, align 8
  %5 = getelementptr inbounds { i64, i32 }, ptr %3, i32 0, i32 1
  store i32 %1, ptr %5, align 8
  %6 = getelementptr inbounds %struct.Wide, ptr %3, i32 0, i32 0
  %7 = load i64, ptr %6, align 8
  ret i64 %7
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
