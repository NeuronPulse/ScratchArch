; ModuleID = 'abi-struct-return.c'
source_filename = "abi-struct-return.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%struct.Triple = type { i32, i32, i32 }
%struct.Big = type { i32, i32, i32, i32, i32 }

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca %struct.Triple, align 4
  %3 = alloca { i64, i32 }, align 8
  %4 = alloca %struct.Big, align 4
  %5 = alloca %struct.Big, align 4
  store i32 0, ptr %1, align 4
  %6 = call { i64, i32 } @make_triple(i32 noundef 10)
  store { i64, i32 } %6, ptr %3, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 8 %3, i64 12, i1 false)
  call void @make_big(ptr dead_on_unwind writable sret(%struct.Big) align 4 %4, i32 noundef 20)
  call void @make_big(ptr dead_on_unwind writable sret(%struct.Big) align 4 %5, i32 noundef 30)
  %7 = getelementptr inbounds %struct.Triple, ptr %2, i32 0, i32 2
  %8 = load i32, ptr %7, align 4
  %9 = getelementptr inbounds %struct.Big, ptr %4, i32 0, i32 0
  %10 = load i32, ptr %9, align 4
  %11 = add nsw i32 %8, %10
  %12 = getelementptr inbounds %struct.Big, ptr %5, i32 0, i32 4
  %13 = load i32, ptr %12, align 4
  %14 = add nsw i32 %11, %13
  ret i32 %14
}

; Function Attrs: noinline nounwind uwtable
define internal { i64, i32 } @make_triple(i32 noundef %0) #0 {
  %2 = alloca %struct.Triple, align 4
  %3 = alloca i32, align 4
  %4 = alloca { i64, i32 }, align 8
  store i32 %0, ptr %3, align 4
  %5 = load i32, ptr %3, align 4
  %6 = getelementptr inbounds %struct.Triple, ptr %2, i32 0, i32 0
  store i32 %5, ptr %6, align 4
  %7 = load i32, ptr %3, align 4
  %8 = add nsw i32 %7, 1
  %9 = getelementptr inbounds %struct.Triple, ptr %2, i32 0, i32 1
  store i32 %8, ptr %9, align 4
  %10 = load i32, ptr %3, align 4
  %11 = add nsw i32 %10, 2
  %12 = getelementptr inbounds %struct.Triple, ptr %2, i32 0, i32 2
  store i32 %11, ptr %12, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 8 %4, ptr align 4 %2, i64 12, i1 false)
  %13 = load { i64, i32 }, ptr %4, align 8
  ret { i64, i32 } %13
}

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) #1

; Function Attrs: noinline nounwind uwtable
define internal void @make_big(ptr dead_on_unwind noalias writable sret(%struct.Big) align 4 %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  store i32 %1, ptr %3, align 4
  %4 = load i32, ptr %3, align 4
  %5 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 0
  store i32 %4, ptr %5, align 4
  %6 = load i32, ptr %3, align 4
  %7 = add nsw i32 %6, 1
  %8 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 1
  store i32 %7, ptr %8, align 4
  %9 = load i32, ptr %3, align 4
  %10 = add nsw i32 %9, 2
  %11 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 2
  store i32 %10, ptr %11, align 4
  %12 = load i32, ptr %3, align 4
  %13 = add nsw i32 %12, 3
  %14 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 3
  store i32 %13, ptr %14, align 4
  %15 = load i32, ptr %3, align 4
  %16 = add nsw i32 %15, 4
  %17 = getelementptr inbounds %struct.Big, ptr %0, i32 0, i32 4
  store i32 %16, ptr %17, align 4
  ret void
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
