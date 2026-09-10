; ModuleID = 'abi-struct-param.c'
source_filename = "abi-struct-param.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }
%struct.Big = type { i32, i32, i32, i32, i32 }

@__const.main.p = private unnamed_addr constant %struct.Pair { i32 4, i32 2 }, align 4
@__const.main.b = private unnamed_addr constant %struct.Big { i32 1, i32 2, i32 3, i32 4, i32 5 }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Pair, align 4
  %3 = alloca %struct.Big, align 8
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.p, i64 8, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 @__const.main.b, i64 20, i1 false)
  %4 = load i64, ptr %2, align 4
  %5 = call i32 @pair_sum(i64 %4)
  %6 = call i32 @big_sum(ptr noundef byval(%struct.Big) align 8 %3)
  %7 = add nsw i32 %5, %6
  ret i32 %7
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal i32 @pair_sum(i64 %0) #0 {
  %2 = alloca %struct.Pair, align 4
  store i64 %0, ptr %2, align 4
  %3 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 0
  %4 = load i32, ptr %3, align 4
  %5 = mul nsw i32 %4, 10
  %6 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 1
  %7 = load i32, ptr %6, align 4
  %8 = add nsw i32 %5, %7
  ret i32 %8
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @big_sum(ptr noundef byval(%struct.Big) align 8 %0) #0 {
  %2 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 0
  %3 = load i32, ptr %2, align 8
  %4 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 1
  %5 = load i32, ptr %4, align 4
  %6 = add nsw i32 %3, %5
  %7 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 2
  %8 = load i32, ptr %7, align 8
  %9 = add nsw i32 %6, %8
  %10 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 3
  %11 = load i32, ptr %10, align 4
  %12 = add nsw i32 %9, %11
  %13 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 4
  %14 = load i32, ptr %13, align 8
  %15 = add nsw i32 %12, %14
  ret i32 %15
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
