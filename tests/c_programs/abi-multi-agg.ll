; ModuleID = 'abi-multi-agg.c'
source_filename = "abi-multi-agg.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Big = type { i32, i32, i32, i32, i32 }
%struct.Pair = type { i32, i32 }

@__const.main.x = private unnamed_addr constant %struct.Big { i32 1, i32 2, i32 3, i32 4, i32 5 }, align 4
@__const.main.y = private unnamed_addr constant %struct.Big { i32 10, i32 20, i32 30, i32 40, i32 50 }, align 4
@__const.main.p = private unnamed_addr constant %struct.Pair { i32 6, i32 7 }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Big, align 8
  %3 = alloca %struct.Big, align 8
  %4 = alloca %struct.Pair, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.x, i64 20, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 @__const.main.y, i64 20, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %4, ptr align 4 @__const.main.p, i64 8, i1 false)
  %5 = call i32 @combine(ptr noundef byval(%struct.Big) align 8 %2, ptr noundef byval(%struct.Big) align 8 %3)
  %6 = load i64, ptr %4, align 4
  %7 = call i32 @pair_and_big(i64 %6, ptr noundef byval(%struct.Big) align 8 %2)
  %8 = add nsw i32 %5, %7
  ret i32 %8
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal i32 @combine(ptr noundef byval(%struct.Big) align 8 %0, ptr noundef byval(%struct.Big) align 8 %1) #0 {
  %3 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 0
  %4 = load i32, ptr %3, align 8
  %5 = getelementptr inbounds %struct.Big, ptr %1, i32 0, i32 4
  %6 = load i32, ptr %5, align 8
  %7 = add nsw i32 %4, %6
  ret i32 %7
}

; Function Attrs: noinline nounwind uwtable
define internal i32 @pair_and_big(i64 %0, ptr noundef byval(%struct.Big) align 8 %1) #0 {
  %3 = alloca %struct.Pair, align 4
  store i64 %0, ptr %3, align 4
  %4 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 0
  %5 = load i32, ptr %4, align 4
  %6 = mul nsw i32 %5, 10
  %7 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 1
  %8 = load i32, ptr %7, align 4
  %9 = add nsw i32 %6, %8
  %10 = getelementptr inbounds %struct.Big, ptr %1, i32 0, i32 0
  %11 = load i32, ptr %10, align 8
  %12 = add nsw i32 %9, %11
  ret i32 %12
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
