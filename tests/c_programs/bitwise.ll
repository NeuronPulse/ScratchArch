; ModuleID = 'bitwise.c'
source_filename = "bitwise.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4
  %3 = call i32 @band(i32 noundef 15, i32 noundef 243)
  %4 = mul nsw i32 %3, 1
  %5 = load i32, ptr %2, align 4
  %6 = add nsw i32 %5, %4
  store i32 %6, ptr %2, align 4
  %7 = call i32 @bor(i32 noundef 15, i32 noundef 48)
  %8 = mul nsw i32 %7, 2
  %9 = load i32, ptr %2, align 4
  %10 = add nsw i32 %9, %8
  store i32 %10, ptr %2, align 4
  %11 = call i32 @bxor(i32 noundef 15, i32 noundef 51)
  %12 = mul nsw i32 %11, 4
  %13 = load i32, ptr %2, align 4
  %14 = add nsw i32 %13, %12
  store i32 %14, ptr %2, align 4
  %15 = call i32 @shl32(i32 noundef 1, i32 noundef 5)
  %16 = mul nsw i32 %15, 8
  %17 = load i32, ptr %2, align 4
  %18 = add nsw i32 %17, %16
  store i32 %18, ptr %2, align 4
  %19 = call i32 @lshr32(i32 noundef 240, i32 noundef 4)
  %20 = mul nsw i32 %19, 16
  %21 = load i32, ptr %2, align 4
  %22 = add nsw i32 %21, %20
  store i32 %22, ptr %2, align 4
  %23 = call i32 @ashr32(i32 noundef -8, i32 noundef 1)
  %24 = mul nsw i32 %23, 32
  %25 = load i32, ptr %2, align 4
  %26 = add nsw i32 %25, %24
  store i32 %26, ptr %2, align 4
  %27 = call i32 @ashr8(i32 noundef -64, i32 noundef 4)
  %28 = mul nsw i32 %27, 64
  %29 = load i32, ptr %2, align 4
  %30 = add nsw i32 %29, %28
  store i32 %30, ptr %2, align 4
  %31 = call i64 @and64(i64 noundef 1311768467463790320, i64 noundef -4294967296)
  %32 = ashr i64 %31, 32
  %33 = trunc i64 %32 to i32
  %34 = call i32 @b8(i32 noundef %33)
  %35 = mul nsw i32 %34, 128
  %36 = load i32, ptr %2, align 4
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %2, align 4
  %38 = call i64 @xor64(i64 noundef -1, i64 noundef 71777214294589695)
  %39 = ashr i64 %38, 32
  %40 = trunc i64 %39 to i32
  %41 = call i32 @b8(i32 noundef %40)
  %42 = mul nsw i32 %41, 256
  %43 = load i32, ptr %2, align 4
  %44 = add nsw i32 %43, %42
  store i32 %44, ptr %2, align 4
  %45 = call i64 @shl64(i64 noundef 1, i64 noundef 40)
  %46 = trunc i64 %45 to i32
  %47 = call i32 @b8(i32 noundef %46)
  %48 = mul nsw i32 %47, 512
  %49 = load i32, ptr %2, align 4
  %50 = add nsw i32 %49, %48
  store i32 %50, ptr %2, align 4
  %51 = call i64 @ashr64(i64 noundef -4294967296, i64 noundef 32)
  %52 = trunc i64 %51 to i32
  %53 = call i32 @b8(i32 noundef %52)
  %54 = mul nsw i32 %53, 1024
  %55 = load i32, ptr %2, align 4
  %56 = add nsw i32 %55, %54
  store i32 %56, ptr %2, align 4
  %57 = load i32, ptr %2, align 4
  ret i32 %57
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @band(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = and i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @bor(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = or i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @bxor(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = xor i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @shl32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = shl i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @lshr32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = lshr i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @ashr32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %5 = load i32, ptr %3, align 4
  %6 = load i32, ptr %4, align 4
  %7 = ashr i32 %5, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @ashr8(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i8, align 1
  store i32 %0, ptr %3, align 4
  store i32 %1, ptr %4, align 4
  %6 = load i32, ptr %3, align 4
  %7 = trunc i32 %6 to i8
  store i8 %7, ptr %5, align 1
  %8 = load i8, ptr %5, align 1
  %9 = sext i8 %8 to i32
  %10 = load i32, ptr %4, align 4
  %11 = ashr i32 %9, %10
  %12 = trunc i32 %11 to i8
  %13 = sext i8 %12 to i32
  %14 = and i32 %13, 255
  ret i32 %14
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @b8(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  %3 = load i32, ptr %2, align 4
  %4 = and i32 %3, 255
  ret i32 %4
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @and64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = and i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @xor64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = xor i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @shl64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = shl i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @ashr64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = ashr i64 %5, %6
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
