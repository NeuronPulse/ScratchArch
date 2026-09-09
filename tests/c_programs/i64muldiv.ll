; ModuleID = 'tests/c_programs/i64muldiv.c'
source_filename = "tests/c_programs/i64muldiv.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i64, align 8
  %6 = alloca i64, align 8
  %7 = alloca i64, align 8
  %8 = alloca i64, align 8
  %9 = alloca i64, align 8
  %10 = alloca i64, align 8
  %11 = alloca i64, align 8
  %12 = alloca i64, align 8
  %13 = alloca i64, align 8
  %14 = alloca i64, align 8
  store i32 0, ptr %1, align 4
  %15 = call i64 @mmul_u(i64 noundef 4294967297, i64 noundef 4294967297)
  store i64 %15, ptr %2, align 8
  %16 = call i64 @mmul_u(i64 noundef 4294967295, i64 noundef 4294967295)
  store i64 %16, ptr %3, align 8
  %17 = call i64 @mmul_s(i64 noundef -10, i64 noundef -10)
  store i64 %17, ptr %4, align 8
  %18 = call i64 @udiv64(i64 noundef 4294967296, i64 noundef 3)
  store i64 %18, ptr %5, align 8
  %19 = call i64 @urem64(i64 noundef 4294967296, i64 noundef 3)
  store i64 %19, ptr %6, align 8
  %20 = call i64 @udiv64(i64 noundef 9223372036854775807, i64 noundef 4294967296)
  store i64 %20, ptr %7, align 8
  %21 = call i64 @urem64(i64 noundef 9223372036854775807, i64 noundef 4294967296)
  store i64 %21, ptr %8, align 8
  %22 = call i64 @sdiv64(i64 noundef -10, i64 noundef 3)
  store i64 %22, ptr %9, align 8
  %23 = call i64 @srem64(i64 noundef -10, i64 noundef 3)
  store i64 %23, ptr %10, align 8
  %24 = call i64 @sdiv64(i64 noundef 10, i64 noundef -3)
  store i64 %24, ptr %11, align 8
  %25 = call i64 @srem64(i64 noundef 10, i64 noundef -3)
  store i64 %25, ptr %12, align 8
  %26 = call i64 @sdiv64(i64 noundef -9223372036854775808, i64 noundef 2)
  store i64 %26, ptr %13, align 8
  store i64 0, ptr %14, align 8
  %27 = load i64, ptr %2, align 8
  %28 = lshr i64 %27, 32
  %29 = mul nsw i64 1000003, %28
  %30 = load i64, ptr %2, align 8
  %31 = add nsw i64 %29, %30
  %32 = load i64, ptr %14, align 8
  %33 = add nsw i64 %32, %31
  store i64 %33, ptr %14, align 8
  %34 = load i64, ptr %3, align 8
  %35 = lshr i64 %34, 32
  %36 = mul nsw i64 1000003, %35
  %37 = load i64, ptr %3, align 8
  %38 = add nsw i64 %36, %37
  %39 = load i64, ptr %14, align 8
  %40 = add nsw i64 %39, %38
  store i64 %40, ptr %14, align 8
  %41 = load i64, ptr %5, align 8
  %42 = lshr i64 %41, 32
  %43 = mul nsw i64 1000003, %42
  %44 = load i64, ptr %5, align 8
  %45 = add nsw i64 %43, %44
  %46 = load i64, ptr %14, align 8
  %47 = add nsw i64 %46, %45
  store i64 %47, ptr %14, align 8
  %48 = load i64, ptr %7, align 8
  %49 = lshr i64 %48, 32
  %50 = mul nsw i64 1000003, %49
  %51 = load i64, ptr %7, align 8
  %52 = add nsw i64 %50, %51
  %53 = load i64, ptr %14, align 8
  %54 = add nsw i64 %53, %52
  store i64 %54, ptr %14, align 8
  %55 = load i64, ptr %6, align 8
  %56 = load i64, ptr %8, align 8
  %57 = add i64 %55, %56
  %58 = load i64, ptr %4, align 8
  %59 = add i64 %57, %58
  %60 = load i64, ptr %14, align 8
  %61 = add i64 %60, %59
  store i64 %61, ptr %14, align 8
  %62 = load i64, ptr %9, align 8
  %63 = load i64, ptr %10, align 8
  %64 = add nsw i64 %62, %63
  %65 = load i64, ptr %11, align 8
  %66 = add nsw i64 %64, %65
  %67 = load i64, ptr %12, align 8
  %68 = add nsw i64 %66, %67
  %69 = load i64, ptr %13, align 8
  %70 = add nsw i64 %68, %69
  %71 = load i64, ptr %14, align 8
  %72 = add nsw i64 %71, %70
  store i64 %72, ptr %14, align 8
  %73 = load i64, ptr %14, align 8
  %74 = trunc i64 %73 to i32
  ret i32 %74
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @mmul_u(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = mul i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @mmul_s(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = mul nsw i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @udiv64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = udiv i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @urem64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = urem i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @sdiv64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = sdiv i64 %5, %6
  ret i64 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i64 @srem64(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8
  store i64 %1, ptr %4, align 8
  %5 = load i64, ptr %3, align 8
  %6 = load i64, ptr %4, align 8
  %7 = srem i64 %5, %6
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
