; ModuleID = 'abi-odd-width.c'
source_filename = "abi-odd-width.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Odd = type { i8, i8, i8 }

@__const.main.o = private unnamed_addr constant %struct.Odd { i8 1, i8 2, i8 3 }, align 1

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Odd, align 1
  %3 = alloca i24, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %2, ptr align 1 @__const.main.o, i64 3, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 1 %2, i64 3, i1 false)
  %4 = load i24, ptr %3, align 4
  %5 = call i32 @odd_sum(i24 %4)
  ret i32 %5
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal i32 @odd_sum(i24 %0) #0 {
  %2 = alloca %struct.Odd, align 1
  store i24 %0, ptr %2, align 1
  %3 = getelementptr inbounds %struct.Odd, ptr %2, i32 0, i32 0
  %4 = load i8, ptr %3, align 1
  %5 = sext i8 %4 to i32
  %6 = getelementptr inbounds %struct.Odd, ptr %2, i32 0, i32 1
  %7 = load i8, ptr %6, align 1
  %8 = sext i8 %7 to i32
  %9 = add nsw i32 %5, %8
  %10 = getelementptr inbounds %struct.Odd, ptr %2, i32 0, i32 2
  %11 = load i8, ptr %10, align 1
  %12 = sext i8 %11 to i32
  %13 = add nsw i32 %9, %12
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
