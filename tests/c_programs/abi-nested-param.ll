; ModuleID = 'abi-nested-param.c'
source_filename = "abi-nested-param.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Wide = type { %struct.Pair, %struct.Pair, i32 }
%struct.Pair = type { i32, i32 }

@__const.main.w = private unnamed_addr constant %struct.Wide { %struct.Pair { i32 1, i32 2 }, %struct.Pair { i32 3, i32 4 }, i32 5 }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Wide, align 8
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.w, i64 20, i1 false)
  %5 = call i32 @wide_sum(ptr noundef byval(%struct.Wide) align 8 %2)
  store i32 %5, ptr %3, align 4
  %6 = call i32 @wide_mutate(ptr noundef byval(%struct.Wide) align 8 %2)
  store i32 %6, ptr %4, align 4
  %7 = load i32, ptr %3, align 4
  %8 = load i32, ptr %4, align 4
  %9 = add nsw i32 %7, %8
  %10 = getelementptr inbounds %struct.Wide, ptr %2, i32 0, i32 0
  %11 = getelementptr inbounds %struct.Pair, ptr %10, i32 0, i32 0
  %12 = load i32, ptr %11, align 4
  %13 = add nsw i32 %9, %12
  ret i32 %13
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal i32 @wide_sum(ptr noundef byval(%struct.Wide) align 8 %0) #0 {
  %2 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 0
  %4 = load i32, ptr %3, align 8
  %5 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 0
  %6 = getelementptr inbounds %struct.Pair, ptr %5, i32 0, i32 1
  %7 = load i32, ptr %6, align 4
  %8 = add nsw i32 %4, %7
  %9 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 1
  %10 = getelementptr inbounds %struct.Pair, ptr %9, i32 0, i32 0
  %11 = load i32, ptr %10, align 8
  %12 = add nsw i32 %8, %11
  %13 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 1
  %14 = getelementptr inbounds %struct.Pair, ptr %13, i32 0, i32 1
  %15 = load i32, ptr %14, align 4
  %16 = add nsw i32 %12, %15
  %17 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 2
  %18 = load i32, ptr %17, align 8
  %19 = add nsw i32 %16, %18
  ret i32 %19
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @wide_mutate(ptr noundef byval(%struct.Wide) align 8 %0) #0 {
  %2 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 0
  store i32 100, ptr %3, align 8
  %4 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 0
  %5 = getelementptr inbounds %struct.Pair, ptr %4, i32 0, i32 0
  %6 = load i32, ptr %5, align 8
  %7 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 1
  %8 = getelementptr inbounds %struct.Pair, ptr %7, i32 0, i32 1
  %9 = load i32, ptr %8, align 4
  %10 = add nsw i32 %6, %9
  %11 = getelementptr inbounds %struct.Wide, ptr %0, i32 0, i32 2
  %12 = load i32, ptr %11, align 8
  %13 = add nsw i32 %10, %12
  ret i32 %13
}

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
