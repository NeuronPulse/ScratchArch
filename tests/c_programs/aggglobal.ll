; ModuleID = 'aggglobal.c'
source_filename = "aggglobal.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Pair = type { i32, i32 }
%struct.Big = type { i8, i64, i16 }
%struct.Mixed = type { i32, ptr }

@sym = dso_local global [3 x i8] c"hi\00", align 1
@one = dso_local global %struct.Pair { i32 3, i32 4 }, align 4
@big = dso_local global %struct.Big { i8 5, i64 6, i16 7 }, align 8
@mixed = dso_local global %struct.Mixed { i32 8, ptr @sym }, align 8

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca ptr, align 8
  store i32 0, ptr %1, align 4
  store ptr @big, ptr %2, align 8
  %3 = load i32, ptr @one, align 4
  %4 = load i32, ptr getelementptr inbounds (%struct.Pair, ptr @one, i32 0, i32 1), align 4
  %5 = add nsw i32 %3, %4
  %6 = load i8, ptr @big, align 8
  %7 = sext i8 %6 to i32
  %8 = add nsw i32 %5, %7
  %9 = load i64, ptr getelementptr inbounds (%struct.Big, ptr @big, i32 0, i32 1), align 8
  %10 = trunc i64 %9 to i32
  %11 = add nsw i32 %8, %10
  %12 = load i16, ptr getelementptr inbounds (%struct.Big, ptr @big, i32 0, i32 2), align 8
  %13 = sext i16 %12 to i32
  %14 = add nsw i32 %11, %13
  %15 = load i32, ptr @mixed, align 8
  %16 = add nsw i32 %14, %15
  %17 = load ptr, ptr getelementptr inbounds (%struct.Mixed, ptr @mixed, i32 0, i32 1), align 8
  %18 = getelementptr inbounds i8, ptr %17, i64 0
  %19 = load i8, ptr %18, align 1
  %20 = sext i8 %19 to i32
  %21 = add nsw i32 %16, %20
  %22 = add nsw i32 %21, 24
  %23 = load ptr, ptr %2, align 8
  %24 = getelementptr inbounds i8, ptr %23, i64 0
  %25 = load i8, ptr %24, align 1
  %26 = sext i8 %25 to i32
  %27 = icmp eq i32 %26, 5
  %28 = zext i1 %27 to i32
  %29 = add nsw i32 %22, %28
  ret i32 %29
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
