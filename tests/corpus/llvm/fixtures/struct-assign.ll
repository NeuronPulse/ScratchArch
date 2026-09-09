; ModuleID = 'struct-assign.c'
source_filename = "struct-assign.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Pair, align 4
  %3 = alloca %struct.Pair, align 4
  store i32 0, ptr %1, align 4
  %4 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 0
  store i32 4, ptr %4, align 4
  %5 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 1
  store i32 2, ptr %5, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 %2, i64 8, i1 false)
  %6 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 0
  store i32 5, ptr %6, align 4
  %7 = call i32 @combine(ptr noundef %3)
  %8 = getelementptr inbounds %struct.Pair, ptr %2, i32 0, i32 0
  %9 = load i32, ptr %8, align 4
  %10 = add nsw i32 %7, %9
  ret i32 %10
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal i32 @combine(ptr noundef %0) #0 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8
  %3 = load ptr, ptr %2, align 8
  %4 = getelementptr inbounds %struct.Pair, ptr %3, i32 0, i32 0
  %5 = load i32, ptr %4, align 4
  %6 = mul nsw i32 %5, 10
  %7 = load ptr, ptr %2, align 8
  %8 = getelementptr inbounds %struct.Pair, ptr %7, i32 0, i32 1
  %9 = load i32, ptr %8, align 4
  %10 = add nsw i32 %6, %9
  ret i32 %10
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
