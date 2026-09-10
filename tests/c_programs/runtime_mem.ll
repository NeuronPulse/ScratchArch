; ModuleID = 'runtime_mem.c'
source_filename = "runtime_mem.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [8 x i8], align 1
  %3 = alloca [8 x i8], align 1
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca [4 x i8], align 1
  %8 = alloca [4 x i8], align 1
  %9 = alloca [8 x i8], align 1
  store i32 0, ptr %1, align 4
  store i32 5, ptr %5, align 4
  store i32 0, ptr %6, align 4
  store i32 0, ptr %4, align 4
  br label %10

10:                                               ; preds = %20, %0
  %11 = load i32, ptr %4, align 4
  %12 = icmp slt i32 %11, 8
  br i1 %12, label %13, label %23

13:                                               ; preds = %10
  %14 = load i32, ptr %4, align 4
  %15 = add nsw i32 %14, 1
  %16 = trunc i32 %15 to i8
  %17 = load i32, ptr %4, align 4
  %18 = sext i32 %17 to i64
  %19 = getelementptr inbounds [8 x i8], ptr %2, i64 0, i64 %18
  store i8 %16, ptr %19, align 1
  br label %20

20:                                               ; preds = %13
  %21 = load i32, ptr %4, align 4
  %22 = add nsw i32 %21, 1
  store i32 %22, ptr %4, align 4
  br label %10, !llvm.loop !6

23:                                               ; preds = %10
  store i32 0, ptr %4, align 4
  br label %24

24:                                               ; preds = %31, %23
  %25 = load i32, ptr %4, align 4
  %26 = icmp slt i32 %25, 8
  br i1 %26, label %27, label %34

27:                                               ; preds = %24
  %28 = load i32, ptr %4, align 4
  %29 = sext i32 %28 to i64
  %30 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 %29
  store i8 0, ptr %30, align 1
  br label %31

31:                                               ; preds = %27
  %32 = load i32, ptr %4, align 4
  %33 = add nsw i32 %32, 1
  store i32 %33, ptr %4, align 4
  br label %24, !llvm.loop !8

34:                                               ; preds = %24
  %35 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %36 = getelementptr inbounds [8 x i8], ptr %2, i64 0, i64 0
  %37 = load i32, ptr %5, align 4
  %38 = call ptr @__scratcharch_memcpy(ptr noundef %35, ptr noundef %36, i32 noundef %37)
  %39 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %40 = load i8, ptr %39, align 1
  %41 = sext i8 %40 to i32
  %42 = icmp eq i32 %41, 1
  br i1 %42, label %43, label %56

43:                                               ; preds = %34
  %44 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 4
  %45 = load i8, ptr %44, align 1
  %46 = sext i8 %45 to i32
  %47 = icmp eq i32 %46, 5
  br i1 %47, label %48, label %56

48:                                               ; preds = %43
  %49 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 5
  %50 = load i8, ptr %49, align 1
  %51 = sext i8 %50 to i32
  %52 = icmp eq i32 %51, 0
  br i1 %52, label %53, label %56

53:                                               ; preds = %48
  %54 = load i32, ptr %6, align 4
  %55 = add nsw i32 %54, 1
  store i32 %55, ptr %6, align 4
  br label %56

56:                                               ; preds = %53, %48, %43, %34
  %57 = getelementptr inbounds [8 x i8], ptr %2, i64 0, i64 0
  %58 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %59 = load i32, ptr %5, align 4
  %60 = call i32 @__scratcharch_memcmp(ptr noundef %57, ptr noundef %58, i32 noundef %59)
  %61 = icmp eq i32 %60, 0
  br i1 %61, label %62, label %65

62:                                               ; preds = %56
  %63 = load i32, ptr %6, align 4
  %64 = add nsw i32 %63, 2
  store i32 %64, ptr %6, align 4
  br label %65

65:                                               ; preds = %62, %56
  %66 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %67 = load i32, ptr %5, align 4
  %68 = call ptr @__scratcharch_memset(ptr noundef %66, i32 noundef 7, i32 noundef %67)
  %69 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %70 = load i8, ptr %69, align 1
  %71 = sext i8 %70 to i32
  %72 = icmp eq i32 %71, 7
  br i1 %72, label %73, label %86

73:                                               ; preds = %65
  %74 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 4
  %75 = load i8, ptr %74, align 1
  %76 = sext i8 %75 to i32
  %77 = icmp eq i32 %76, 7
  br i1 %77, label %78, label %86

78:                                               ; preds = %73
  %79 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 5
  %80 = load i8, ptr %79, align 1
  %81 = sext i8 %80 to i32
  %82 = icmp eq i32 %81, 0
  br i1 %82, label %83, label %86

83:                                               ; preds = %78
  %84 = load i32, ptr %6, align 4
  %85 = add nsw i32 %84, 4
  store i32 %85, ptr %6, align 4
  br label %86

86:                                               ; preds = %83, %78, %73, %65
  %87 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %88 = getelementptr inbounds i8, ptr %87, i64 1
  %89 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 0
  %90 = call ptr @__scratcharch_memmove(ptr noundef %88, ptr noundef %89, i32 noundef 4)
  %91 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 1
  %92 = load i8, ptr %91, align 1
  %93 = sext i8 %92 to i32
  %94 = icmp eq i32 %93, 7
  br i1 %94, label %95, label %103

95:                                               ; preds = %86
  %96 = getelementptr inbounds [8 x i8], ptr %3, i64 0, i64 4
  %97 = load i8, ptr %96, align 1
  %98 = sext i8 %97 to i32
  %99 = icmp eq i32 %98, 7
  br i1 %99, label %100, label %103

100:                                              ; preds = %95
  %101 = load i32, ptr %6, align 4
  %102 = add nsw i32 %101, 8
  store i32 %102, ptr %6, align 4
  br label %103

103:                                              ; preds = %100, %95, %86
  %104 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 0
  store i8 97, ptr %104, align 1
  %105 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 1
  store i8 98, ptr %105, align 1
  %106 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 2
  store i8 99, ptr %106, align 1
  %107 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 3
  store i8 0, ptr %107, align 1
  %108 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 0
  store i8 97, ptr %108, align 1
  %109 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 1
  store i8 98, ptr %109, align 1
  %110 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 2
  store i8 100, ptr %110, align 1
  %111 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 3
  store i8 0, ptr %111, align 1
  %112 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 0
  %113 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 0
  %114 = call i32 @__scratcharch_strcmp(ptr noundef %112, ptr noundef %113)
  %115 = icmp slt i32 %114, 0
  br i1 %115, label %116, label %119

116:                                              ; preds = %103
  %117 = load i32, ptr %6, align 4
  %118 = add nsw i32 %117, 16
  store i32 %118, ptr %6, align 4
  br label %119

119:                                              ; preds = %116, %103
  %120 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 0
  %121 = getelementptr inbounds [4 x i8], ptr %7, i64 0, i64 0
  %122 = call ptr @__scratcharch_strcpy(ptr noundef %120, ptr noundef %121)
  %123 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 0
  %124 = load i8, ptr %123, align 1
  %125 = sext i8 %124 to i32
  %126 = icmp eq i32 %125, 97
  br i1 %126, label %127, label %140

127:                                              ; preds = %119
  %128 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 2
  %129 = load i8, ptr %128, align 1
  %130 = sext i8 %129 to i32
  %131 = icmp eq i32 %130, 99
  br i1 %131, label %132, label %140

132:                                              ; preds = %127
  %133 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 3
  %134 = load i8, ptr %133, align 1
  %135 = sext i8 %134 to i32
  %136 = icmp eq i32 %135, 0
  br i1 %136, label %137, label %140

137:                                              ; preds = %132
  %138 = load i32, ptr %6, align 4
  %139 = add nsw i32 %138, 32
  store i32 %139, ptr %6, align 4
  br label %140

140:                                              ; preds = %137, %132, %127, %119
  %141 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 0
  %142 = call i32 @__scratcharch_strlen(ptr noundef %141)
  %143 = icmp eq i32 %142, 3
  br i1 %143, label %144, label %147

144:                                              ; preds = %140
  %145 = load i32, ptr %6, align 4
  %146 = add nsw i32 %145, 64
  store i32 %146, ptr %6, align 4
  br label %147

147:                                              ; preds = %144, %140
  %148 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 0
  %149 = getelementptr inbounds [4 x i8], ptr %8, i64 0, i64 0
  %150 = call ptr @__scratcharch_strncpy(ptr noundef %148, ptr noundef %149, i32 noundef 6)
  %151 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 0
  %152 = load i8, ptr %151, align 1
  %153 = sext i8 %152 to i32
  %154 = icmp eq i32 %153, 97
  br i1 %154, label %155, label %163

155:                                              ; preds = %147
  %156 = getelementptr inbounds [8 x i8], ptr %9, i64 0, i64 2
  %157 = load i8, ptr %156, align 1
  %158 = sext i8 %157 to i32
  %159 = icmp eq i32 %158, 100
  br i1 %159, label %160, label %163

160:                                              ; preds = %155
  %161 = load i32, ptr %6, align 4
  %162 = add nsw i32 %161, 128
  store i32 %162, ptr %6, align 4
  br label %163

163:                                              ; preds = %160, %155, %147
  %164 = load i32, ptr %6, align 4
  ret i32 %164
}

declare ptr @__scratcharch_memcpy(ptr noundef, ptr noundef, i32 noundef) #1

declare i32 @__scratcharch_memcmp(ptr noundef, ptr noundef, i32 noundef) #1

declare ptr @__scratcharch_memset(ptr noundef, i32 noundef, i32 noundef) #1

declare ptr @__scratcharch_memmove(ptr noundef, ptr noundef, i32 noundef) #1

declare i32 @__scratcharch_strcmp(ptr noundef, ptr noundef) #1

declare ptr @__scratcharch_strcpy(ptr noundef, ptr noundef) #1

declare i32 @__scratcharch_strlen(ptr noundef) #1

declare ptr @__scratcharch_strncpy(ptr noundef, ptr noundef, i32 noundef) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"Debian clang version 19.1.7 (3+b1)"}
!6 = distinct !{!6, !7}
!7 = !{!"llvm.loop.mustprogress"}
!8 = distinct !{!8, !7}
