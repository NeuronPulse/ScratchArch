; ModuleID = 'tests/c_programs/bytes.c'
source_filename = "tests/c_programs/bytes.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@bytes = dso_local constant [4 x i8] c"\11\223D", align 1
@pairs = dso_local constant [2 x i16] [i16 -16657, i16 4660], align 2
@neg = dso_local constant i8 -1, align 1
@gd = dso_local global [4 x i8] c"\AA\BB\CC\DD", align 1

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  %5 = alloca i16, align 2
  %6 = alloca i32, align 4
  %7 = alloca ptr, align 8
  %8 = alloca [2 x i8], align 1
  %9 = alloca i16, align 2
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4
  %10 = load i8, ptr @bytes, align 1
  store i8 %10, ptr %3, align 1
  %11 = load i8, ptr getelementptr inbounds ([4 x i8], ptr @bytes, i64 0, i64 3), align 1
  store i8 %11, ptr %4, align 1
  %12 = load i16, ptr getelementptr inbounds ([2 x i16], ptr @pairs, i64 0, i64 1), align 2
  store i16 %12, ptr %5, align 2
  %13 = load i8, ptr %3, align 1
  %14 = zext i8 %13 to i32
  %15 = load i8, ptr %4, align 1
  %16 = zext i8 %15 to i32
  %17 = add nsw i32 %14, %16
  %18 = load i16, ptr %5, align 2
  %19 = zext i16 %18 to i32
  %20 = and i32 %19, 255
  %21 = add nsw i32 %17, %20
  %22 = load i16, ptr %5, align 2
  %23 = zext i16 %22 to i32
  %24 = ashr i32 %23, 8
  %25 = add nsw i32 %21, %24
  %26 = load i32, ptr %2, align 4
  %27 = add nsw i32 %26, %25
  store i32 %27, ptr %2, align 4
  %28 = load i32, ptr %2, align 4
  %29 = add nsw i32 %28, 1
  store i32 %29, ptr %2, align 4
  store i8 90, ptr getelementptr inbounds ([4 x i8], ptr @gd, i64 0, i64 1), align 1
  %30 = load i8, ptr @gd, align 1
  %31 = zext i8 %30 to i32
  %32 = icmp eq i32 %31, 170
  %33 = zext i1 %32 to i64
  %34 = select i1 %32, i32 1, i32 0
  %35 = load i32, ptr %2, align 4
  %36 = add nsw i32 %35, %34
  store i32 %36, ptr %2, align 4
  %37 = load i8, ptr getelementptr inbounds ([4 x i8], ptr @gd, i64 0, i64 1), align 1
  %38 = zext i8 %37 to i32
  %39 = icmp eq i32 %38, 90
  %40 = zext i1 %39 to i64
  %41 = select i1 %39, i32 2, i32 0
  %42 = load i32, ptr %2, align 4
  %43 = add nsw i32 %42, %41
  store i32 %43, ptr %2, align 4
  %44 = load i8, ptr getelementptr inbounds ([4 x i8], ptr @gd, i64 0, i64 2), align 1
  %45 = zext i8 %44 to i32
  %46 = icmp eq i32 %45, 204
  %47 = zext i1 %46 to i64
  %48 = select i1 %46, i32 4, i32 0
  %49 = load i32, ptr %2, align 4
  %50 = add nsw i32 %49, %48
  store i32 %50, ptr %2, align 4
  %51 = load i8, ptr getelementptr inbounds ([4 x i8], ptr @gd, i64 0, i64 3), align 1
  %52 = zext i8 %51 to i32
  %53 = icmp eq i32 %52, 221
  %54 = zext i1 %53 to i64
  %55 = select i1 %53, i32 8, i32 0
  %56 = load i32, ptr %2, align 4
  %57 = add nsw i32 %56, %55
  store i32 %57, ptr %2, align 4
  store i32 2018915346, ptr %6, align 4
  store ptr %6, ptr %7, align 8
  %58 = load ptr, ptr %7, align 8
  %59 = getelementptr inbounds i8, ptr %58, i64 0
  %60 = load i8, ptr %59, align 1
  %61 = zext i8 %60 to i32
  %62 = icmp eq i32 %61, 18
  %63 = zext i1 %62 to i64
  %64 = select i1 %62, i32 16, i32 0
  %65 = load i32, ptr %2, align 4
  %66 = add nsw i32 %65, %64
  store i32 %66, ptr %2, align 4
  %67 = load ptr, ptr %7, align 8
  %68 = getelementptr inbounds i8, ptr %67, i64 1
  %69 = load i8, ptr %68, align 1
  %70 = zext i8 %69 to i32
  %71 = icmp eq i32 %70, 52
  %72 = zext i1 %71 to i64
  %73 = select i1 %71, i32 32, i32 0
  %74 = load i32, ptr %2, align 4
  %75 = add nsw i32 %74, %73
  store i32 %75, ptr %2, align 4
  %76 = load ptr, ptr %7, align 8
  %77 = getelementptr inbounds i8, ptr %76, i64 2
  %78 = load i8, ptr %77, align 1
  %79 = zext i8 %78 to i32
  %80 = icmp eq i32 %79, 86
  %81 = zext i1 %80 to i64
  %82 = select i1 %80, i32 64, i32 0
  %83 = load i32, ptr %2, align 4
  %84 = add nsw i32 %83, %82
  store i32 %84, ptr %2, align 4
  %85 = load ptr, ptr %7, align 8
  %86 = getelementptr inbounds i8, ptr %85, i64 3
  %87 = load i8, ptr %86, align 1
  %88 = zext i8 %87 to i32
  %89 = icmp eq i32 %88, 120
  %90 = zext i1 %89 to i64
  %91 = select i1 %89, i32 128, i32 0
  %92 = load i32, ptr %2, align 4
  %93 = add nsw i32 %92, %91
  store i32 %93, ptr %2, align 4
  %94 = getelementptr inbounds [2 x i8], ptr %8, i64 0, i64 0
  store i8 -17, ptr %94, align 1
  %95 = getelementptr inbounds [2 x i8], ptr %8, i64 0, i64 1
  store i8 -66, ptr %95, align 1
  %96 = getelementptr inbounds [2 x i8], ptr %8, i64 0, i64 0
  %97 = load i8, ptr %96, align 1
  %98 = zext i8 %97 to i32
  %99 = getelementptr inbounds [2 x i8], ptr %8, i64 0, i64 1
  %100 = load i8, ptr %99, align 1
  %101 = zext i8 %100 to i16
  %102 = zext i16 %101 to i32
  %103 = shl i32 %102, 8
  %104 = or i32 %98, %103
  %105 = trunc i32 %104 to i16
  store i16 %105, ptr %9, align 2
  %106 = load i16, ptr %9, align 2
  %107 = zext i16 %106 to i32
  %108 = icmp eq i32 %107, 48879
  %109 = zext i1 %108 to i64
  %110 = select i1 %108, i32 1, i32 0
  %111 = load i32, ptr %2, align 4
  %112 = add nsw i32 %111, %110
  store i32 %112, ptr %2, align 4
  %113 = load i32, ptr %2, align 4
  ret i32 %113
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
