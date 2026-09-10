; ModuleID = 'abi-nested-return.c'
source_filename = "abi-nested-return.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Nested = type { %struct.Pair, i32 }
%struct.Pair = type { i32, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Nested, align 4
  %3 = alloca { i64, i32 }, align 8
  %4 = alloca %struct.Nested, align 4
  %5 = alloca { i64, i32 }, align 8
  store i32 0, ptr %1, align 4
  %6 = call { i64, i32 } @nested_make(i32 noundef 1, i32 noundef 2, i32 noundef 3)
  store { i64, i32 } %6, ptr %3, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 8 %3, i64 12, i1 false)
  %7 = call { i64, i32 } @nested_make(i32 noundef 4, i32 noundef 5, i32 noundef 6)
  store { i64, i32 } %7, ptr %5, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %4, ptr align 8 %5, i64 12, i1 false)
  %8 = getelementptr inbounds %struct.Nested, ptr %2, i32 0, i32 0
  %9 = getelementptr inbounds %struct.Pair, ptr %8, i32 0, i32 0
  %10 = load i32, ptr %9, align 4
  %11 = mul nsw i32 %10, 100
  %12 = getelementptr inbounds %struct.Nested, ptr %2, i32 0, i32 0
  %13 = getelementptr inbounds %struct.Pair, ptr %12, i32 0, i32 1
  %14 = load i32, ptr %13, align 4
  %15 = mul nsw i32 %14, 10
  %16 = add nsw i32 %11, %15
  %17 = getelementptr inbounds %struct.Nested, ptr %4, i32 0, i32 1
  %18 = load i32, ptr %17, align 4
  %19 = add nsw i32 %16, %18
  ret i32 %19
}

; Function Attrs: noinline nounwind uwtable
define internal { i64, i32 } @nested_make(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca %struct.Nested, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca { i64, i32 }, align 8
  store i32 %0, ptr %5, align 4
  store i32 %1, ptr %6, align 4
  store i32 %2, ptr %7, align 4
  %9 = load i32, ptr %5, align 4
  %10 = getelementptr inbounds %struct.Nested, ptr %4, i32 0, i32 0
  %11 = getelementptr inbounds %struct.Pair, ptr %10, i32 0, i32 0
  store i32 %9, ptr %11, align 4
  %12 = load i32, ptr %6, align 4
  %13 = getelementptr inbounds %struct.Nested, ptr %4, i32 0, i32 0
  %14 = getelementptr inbounds %struct.Pair, ptr %13, i32 0, i32 1
  store i32 %12, ptr %14, align 4
  %15 = load i32, ptr %7, align 4
  %16 = getelementptr inbounds %struct.Nested, ptr %4, i32 0, i32 1
  store i32 %15, ptr %16, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %8, ptr align 4 %4, i64 12, i1 false)
  %17 = load { i64, i32 }, ptr %8, align 8
  ret { i64, i32 } %17
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
