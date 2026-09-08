; ModuleID = 'tests/c_programs/signedcmp.c'
source_filename = "tests/c_programs/signedcmp.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @lt(i32 noundef -5, i32 noundef 1)
  %3 = call i32 @lt(i32 noundef -5, i32 noundef -1)
  %4 = mul nsw i32 %3, 2
  %5 = add nsw i32 %2, %4
  %6 = call i32 @lt(i32 noundef -1, i32 noundef -5)
  %7 = mul nsw i32 %6, 4
  %8 = add nsw i32 %5, %7
  %9 = call i32 @le(i32 noundef -5, i32 noundef -5)
  %10 = mul nsw i32 %9, 8
  %11 = add nsw i32 %8, %10
  %12 = call i32 @gt(i32 noundef 1, i32 noundef -5)
  %13 = mul nsw i32 %12, 16
  %14 = add nsw i32 %11, %13
  %15 = call i32 @ge(i32 noundef -1, i32 noundef -5)
  %16 = mul nsw i32 %15, 32
  %17 = add nsw i32 %14, %16
  %18 = call i32 @ge(i32 noundef -5, i32 noundef -1)
  %19 = mul nsw i32 %18, 64
  %20 = add nsw i32 %17, %19
  %21 = call i32 @gt(i32 noundef -3, i32 noundef 2)
  %22 = mul nsw i32 %21, 128
  %23 = add nsw i32 %20, %22
  ret i32 %23
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @lt(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = icmp slt i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @le(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = icmp sle i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @gt(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = icmp sgt i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @ge(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = icmp sge i32 %5, %6
  %8 = zext i1 %7 to i32
  ret i32 %8
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
