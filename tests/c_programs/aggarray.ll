; ModuleID = 'aggarray.c'
source_filename = "aggarray.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Row = type { i32, [6 x i8] }
%struct.Pair = type { i32, i32 }

@__const.main.r = private unnamed_addr constant %struct.Row { i32 9, [6 x i8] c"hello\00" }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [3 x %struct.Pair], align 16
  %3 = alloca %struct.Row, align 4
  %4 = alloca [2 x [3 x i32]], align 16
  store i32 0, ptr %1, align 4
  %5 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 0
  %6 = getelementptr inbounds %struct.Pair, ptr %5, i32 0, i32 0
  store i32 1, ptr %6, align 16
  %7 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 0
  %8 = getelementptr inbounds %struct.Pair, ptr %7, i32 0, i32 1
  store i32 2, ptr %8, align 4
  %9 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 1
  %10 = getelementptr inbounds %struct.Pair, ptr %9, i32 0, i32 0
  store i32 3, ptr %10, align 8
  %11 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 1
  %12 = getelementptr inbounds %struct.Pair, ptr %11, i32 0, i32 1
  store i32 4, ptr %12, align 4
  %13 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 2
  %14 = getelementptr inbounds %struct.Pair, ptr %13, i32 0, i32 0
  store i32 5, ptr %14, align 16
  %15 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 2
  %16 = getelementptr inbounds %struct.Pair, ptr %15, i32 0, i32 1
  store i32 6, ptr %16, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 @__const.main.r, i64 12, i1 false)
  %17 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 2
  %18 = getelementptr inbounds %struct.Pair, ptr %17, i32 0, i32 1
  %19 = load i32, ptr %18, align 4
  %20 = getelementptr inbounds [2 x [3 x i32]], ptr %4, i64 0, i64 1
  %21 = getelementptr inbounds [3 x i32], ptr %20, i64 0, i64 2
  store i32 %19, ptr %21, align 4
  %22 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 0
  %23 = getelementptr inbounds %struct.Pair, ptr %22, i32 0, i32 0
  %24 = load i32, ptr %23, align 16
  %25 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 1
  %26 = getelementptr inbounds %struct.Pair, ptr %25, i32 0, i32 1
  %27 = load i32, ptr %26, align 4
  %28 = add nsw i32 %24, %27
  %29 = getelementptr inbounds [3 x %struct.Pair], ptr %2, i64 0, i64 2
  %30 = getelementptr inbounds %struct.Pair, ptr %29, i32 0, i32 0
  %31 = load i32, ptr %30, align 16
  %32 = add nsw i32 %28, %31
  %33 = getelementptr inbounds %struct.Row, ptr %3, i32 0, i32 0
  %34 = load i32, ptr %33, align 4
  %35 = add nsw i32 %32, %34
  %36 = getelementptr inbounds %struct.Row, ptr %3, i32 0, i32 1
  %37 = getelementptr inbounds [6 x i8], ptr %36, i64 0, i64 1
  %38 = load i8, ptr %37, align 1
  %39 = sext i8 %38 to i32
  %40 = add nsw i32 %35, %39
  %41 = getelementptr inbounds [2 x [3 x i32]], ptr %4, i64 0, i64 1
  %42 = getelementptr inbounds [3 x i32], ptr %41, i64 0, i64 2
  %43 = load i32, ptr %42, align 4
  %44 = add nsw i32 %40, %43
  %45 = add nsw i32 %44, 12
  ret i32 %45
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nounwind willreturn memory(argmem: readwrite) }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"Debian clang version 19.1.7 (3+b1)"}
