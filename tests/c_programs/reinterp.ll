; ModuleID = 'reinterp.c'
source_filename = "reinterp.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i64, align 8
  %5 = alloca i64, align 8
  %6 = alloca ptr, align 8
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4
  store i32 300, ptr %3, align 4
  %7 = ptrtoint ptr %3 to i64
  store i64 %7, ptr %4, align 8
  %8 = load i64, ptr %4, align 8
  %9 = inttoptr i64 %8 to ptr
  %10 = ptrtoint ptr %9 to i64
  store i64 %10, ptr %5, align 8
  %11 = load i64, ptr %4, align 8
  %12 = load i64, ptr %5, align 8
  %13 = icmp eq i64 %11, %12
  %14 = zext i1 %13 to i64
  %15 = select i1 %13, i32 1, i32 0
  %16 = load i32, ptr %2, align 4
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %2, align 4
  %18 = load i64, ptr %4, align 8
  %19 = call i32 @read_at(i64 noundef %18)
  %20 = icmp eq i32 %19, 300
  %21 = zext i1 %20 to i64
  %22 = select i1 %20, i32 2, i32 0
  %23 = load i32, ptr %2, align 4
  %24 = add nsw i32 %23, %22
  store i32 %24, ptr %2, align 4
  %25 = load i64, ptr %5, align 8
  %26 = call i32 @read_at(i64 noundef %25)
  %27 = icmp eq i32 %26, 300
  %28 = zext i1 %27 to i64
  %29 = select i1 %27, i32 4, i32 0
  %30 = load i32, ptr %2, align 4
  %31 = add nsw i32 %30, %29
  store i32 %31, ptr %2, align 4
  %32 = ptrtoint ptr %3 to i64
  %33 = inttoptr i64 %32 to ptr
  store ptr %33, ptr %6, align 8
  %34 = load ptr, ptr %6, align 8
  %35 = getelementptr inbounds i8, ptr %34, i64 0
  %36 = load i8, ptr %35, align 1
  %37 = zext i8 %36 to i32
  %38 = icmp eq i32 %37, 44
  %39 = zext i1 %38 to i64
  %40 = select i1 %38, i32 8, i32 0
  %41 = load i32, ptr %2, align 4
  %42 = add nsw i32 %41, %40
  store i32 %42, ptr %2, align 4
  %43 = load ptr, ptr %6, align 8
  %44 = getelementptr inbounds i8, ptr %43, i64 1
  %45 = load i8, ptr %44, align 1
  %46 = zext i8 %45 to i32
  %47 = icmp eq i32 %46, 1
  %48 = zext i1 %47 to i64
  %49 = select i1 %47, i32 16, i32 0
  %50 = load i32, ptr %2, align 4
  %51 = add nsw i32 %50, %49
  store i32 %51, ptr %2, align 4
  %52 = load i32, ptr %2, align 4
  %53 = add nsw i32 300, %52
  ret i32 %53
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @read_at(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8
  %3 = load i64, ptr %2, align 8
  %4 = inttoptr i64 %3 to ptr
  %5 = load i32, ptr %4, align 4
  ret i32 %5
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
