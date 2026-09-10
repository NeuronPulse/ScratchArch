; ModuleID = 'aggnested.c'
source_filename = "aggnested.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Table = type { [2 x %struct.Line], i32 }
%struct.Line = type { %struct.Pair, %struct.Pair }
%struct.Pair = type { i32, i32 }

@t = dso_local global %struct.Table { [2 x %struct.Line] [%struct.Line { %struct.Pair { i32 1, i32 2 }, %struct.Pair { i32 3, i32 4 } }, %struct.Line { %struct.Pair { i32 5, i32 6 }, %struct.Pair { i32 7, i32 8 } }], i32 9 }, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load i32, ptr @t, align 4
  %3 = load i32, ptr getelementptr inbounds (%struct.Pair, ptr getelementptr inbounds (%struct.Line, ptr @t, i32 0, i32 1), i32 0, i32 1), align 4
  %4 = add nsw i32 %2, %3
  %5 = load i32, ptr getelementptr inbounds ([2 x %struct.Line], ptr @t, i64 0, i64 1), align 4
  %6 = add nsw i32 %4, %5
  %7 = load i32, ptr getelementptr inbounds (%struct.Pair, ptr getelementptr inbounds (%struct.Line, ptr getelementptr inbounds ([2 x %struct.Line], ptr @t, i64 0, i64 1), i32 0, i32 1), i32 0, i32 1), align 4
  %8 = add nsw i32 %6, %7
  %9 = load i32, ptr getelementptr inbounds (%struct.Table, ptr @t, i32 0, i32 1), align 4
  %10 = add nsw i32 %8, %9
  %11 = add nsw i32 %10, 36
  ret i32 %11
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
