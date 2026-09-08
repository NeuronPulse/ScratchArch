; VM-backend globals fixture. Every data element is word-granular (i32/i64/ptr)
; and aligned, so the static segment is VM-exact (StaticData.word_exact): the
; driver seeds it and the 32-bit-word VM loads/stores round-trip exactly. The
; interpreter (semantic reference) and VM must agree on the result, 47.
;
; Exercises: mutable i32 global, pointer relocation (@pc -> @counter), writes
; through a loaded pointer, an inline getelementptr constant expression into a
; global array, and a two-limb i64 global compare.

@counter = global i32 41
@pc = global ptr @counter
@table = global [3 x i32] [i32 1, i32 2, i32 4]
@wide = global i64 4294967296

define i32 @main() {
entry:
  %p = load ptr, ptr @pc
  %old = load i32, ptr %p
  %new = add i32 %old, 1
  store i32 %new, ptr %p
  %c = load i32, ptr @counter
  %t = load i32, ptr getelementptr inbounds ([3 x i32], ptr @table, i64 0, i64 2)
  %w = load i64, ptr @wide
  %ok = icmp eq i64 %w, 4294967296
  %ok32 = zext i1 %ok to i32
  %s1 = add i32 %c, %t
  %s2 = add i32 %s1, %ok32
  ret i32 %s2
}
