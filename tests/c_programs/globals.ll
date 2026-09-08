; ModuleID = 'tests/c_programs/globals.c'
source_filename = "tests/c_programs/globals.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@counter = dso_local global i32 41, align 4
@.str = private unnamed_addr constant [3 x i8] c"hi\00", align 1
@greeting = dso_local global ptr @.str, align 8
@table = internal global [3 x i32] [i32 7, i32 8, i32 9], align 4
@byte_val = internal global i8 -56, align 1
@msg = internal global [16 x i8] c"ok\00\00\00\00\00\00\00\00\00\00\00\00\00\00", align 16
@big = internal global i64 -4294967297, align 8

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @read_and_bump() #0 {
  %1 = load i32, ptr @counter, align 4
  %2 = add nsw i32 %1, 1
  store i32 %2, ptr @counter, align 4
  ret i32 %1
}

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @init_only() #0 {
  %1 = load i32, ptr @counter, align 4
  %2 = load i32, ptr getelementptr inbounds ([3 x i32], ptr @table, i64 0, i64 2), align 4
  %3 = add nsw i32 %1, %2
  ret i32 %3
}

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %8 = call i32 @init_only()
  store i32 %8, ptr %2, align 4
  %9 = call i32 @read_and_bump()
  store i32 %9, ptr %3, align 4
  %10 = load ptr, ptr @greeting, align 8
  %11 = getelementptr inbounds i8, ptr %10, i64 1
  %12 = load i8, ptr %11, align 1
  %13 = sext i8 %12 to i32
  %14 = icmp eq i32 %13, 105
  %15 = zext i1 %14 to i64
  %16 = select i1 %14, i32 1, i32 0
  store i32 %16, ptr %4, align 4
  %17 = load i8, ptr @byte_val, align 1
  %18 = zext i8 %17 to i32
  %19 = icmp eq i32 %18, 200
  %20 = zext i1 %19 to i64
  %21 = select i1 %19, i32 1, i32 0
  store i32 %21, ptr %5, align 4
  %22 = load i8, ptr getelementptr inbounds ([16 x i8], ptr @msg, i64 0, i64 1), align 1
  %23 = sext i8 %22 to i32
  %24 = icmp eq i32 %23, 107
  %25 = zext i1 %24 to i64
  %26 = select i1 %24, i32 1, i32 0
  store i32 %26, ptr %6, align 4
  %27 = load i64, ptr @big, align 8
  %28 = icmp eq i64 %27, -4294967297
  %29 = zext i1 %28 to i64
  %30 = select i1 %28, i32 1, i32 0
  store i32 %30, ptr %7, align 4
  %31 = load i32, ptr %2, align 4
  %32 = load i32, ptr %3, align 4
  %33 = add nsw i32 %31, %32
  %34 = load i32, ptr %4, align 4
  %35 = add nsw i32 %33, %34
  %36 = load i32, ptr %5, align 4
  %37 = add nsw i32 %35, %36
  %38 = load i32, ptr %6, align 4
  %39 = add nsw i32 %37, %38
  %40 = load i32, ptr %7, align 4
  %41 = add nsw i32 %39, %40
  ret i32 %41
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
