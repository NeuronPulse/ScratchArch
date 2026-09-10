; ModuleID = 'aggstruct.c'
source_filename = "aggstruct.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }
%struct.Nest = type { %struct.Pair, i32 }
%struct.Big = type { i8, i64, i16 }

@__const.main.p = private unnamed_addr constant %struct.Pair { i32 3, i32 4 }, align 4
@__const.main.n = private unnamed_addr constant %struct.Nest { %struct.Pair { i32 5, i32 6 }, i32 7 }, align 4
@__const.main.g = private unnamed_addr constant %struct.Big { i8 1, i64 2, i16 3 }, align 8

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Pair, align 4
  %3 = alloca %struct.Pair, align 4
  %4 = alloca %struct.Nest, align 4
  %5 = alloca %struct.Nest, align 4
  %6 = alloca %struct.Big, align 8
  %7 = alloca ptr, align 8
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.p, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 %2, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %4, ptr align 4 @__const.main.n, i64 12, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %5, ptr align 4 %4, i64 12, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %6, ptr align 8 @__const.main.g, i64 24, i1 false)
  store ptr %6, ptr %7, align 8
  %8 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 0
  %9 = load i32, ptr %8, align 4
  %10 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 1
  %11 = load i32, ptr %10, align 4
  %12 = add nsw i32 %9, %11
  %13 = getelementptr inbounds %struct.Nest, ptr %5, i32 0, i32 0
  %14 = getelementptr inbounds %struct.Pair, ptr %13, i32 0, i32 0
  %15 = load i32, ptr %14, align 4
  %16 = add nsw i32 %12, %15
  %17 = getelementptr inbounds %struct.Nest, ptr %5, i32 0, i32 0
  %18 = getelementptr inbounds %struct.Pair, ptr %17, i32 0, i32 1
  %19 = load i32, ptr %18, align 4
  %20 = add nsw i32 %16, %19
  %21 = getelementptr inbounds %struct.Nest, ptr %5, i32 0, i32 1
  %22 = load i32, ptr %21, align 4
  %23 = add nsw i32 %20, %22
  %24 = getelementptr inbounds %struct.Big, ptr %6, i32 0, i32 0
  %25 = load i8, ptr %24, align 8
  %26 = sext i8 %25 to i32
  %27 = add nsw i32 %23, %26
  %28 = getelementptr inbounds %struct.Big, ptr %6, i32 0, i32 1
  %29 = load i64, ptr %28, align 8
  %30 = trunc i64 %29 to i32
  %31 = add nsw i32 %27, %30
  %32 = getelementptr inbounds %struct.Big, ptr %6, i32 0, i32 2
  %33 = load i16, ptr %32, align 8
  %34 = sext i16 %33 to i32
  %35 = add nsw i32 %31, %34
  %36 = add nsw i32 %35, 24
  %37 = load ptr, ptr %7, align 8
  %38 = getelementptr inbounds i8, ptr %37, i64 0
  %39 = load i8, ptr %38, align 1
  %40 = sext i8 %39 to i32
  %41 = icmp eq i32 %40, 1
  %42 = zext i1 %41 to i32
  %43 = add nsw i32 %36, %42
  ret i32 %43
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
