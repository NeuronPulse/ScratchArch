//! LLVM memory-intrinsic tests (`llvm.memcpy`/`llvm.memmove`/`llvm.memset`).
//!
//! Part 4 of the v0.2 LLVM compatibility work: real clang emits these from the
//! standard library `memcpy`/`memmove`/`memset` builtins. The translator keeps
//! them as ordinary SAIR `Call`s to an undeclared function; the interpreter
//! resolves the `llvm.mem*` prefix against its flat byte memory. These tests
//! run the committed clang fixture `tests/c_programs/memintrin.ll` plus a
//! synthetic overlapping-`memmove` case through the interpreter.
//!
//! `memcpy` between non-overlapping regions and `memmove` between overlapping
//! regions must both be well defined; `memcpy` is undefined for overlap, so we
//! only exercise it on disjoint regions.

use std::path::PathBuf;

use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_sair_interpreter::RuntimeValue;

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/scratcharch-llvm
    p.pop(); // project root
    p
}

fn run_ir(ir: &str) -> Result<RuntimeValue, String> {
    let module = translate_llvm(ir).map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp
        .run()
        .map_err(|e| format!("interpreter: {:?}", e))
        .map(|v| v.unwrap_or(RuntimeValue::I32(0)))
}

fn run_fixture(name: &str) -> Result<RuntimeValue, String> {
    let path = project_root().join("tests").join("c_programs").join(format!("{name}.ll"));
    let ir = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    run_ir(&ir)
}

#[test]
fn test_memintrin_fixture_copy_move_set() {
    // memcpy {1,2,3} -> dst, memmove &dst[1]=&dst[0] (overlap), then memset
    // dst[1..3] = 0. Expect 1 + 0 + 0.
    match run_fixture("memintrin").expect("memintrin.ll should translate and run") {
        RuntimeValue::I32(v) => assert_eq!(v, 1, "memintrin result"),
        other => panic!("expected I32, got {:?}", other),
    }
}

#[test]
fn test_memmove_reverse_overlap_is_well_defined() {
    // memmove where dst < src with overlapping regions: bytes must not be
    // clobbered by a forward byte copy. Write a 12-byte buffer via three i32
    // GEP stores, move the middle 4 bytes down by 4, and read them back.
    let ir = r#"
define i32 @main() {
entry:
  %buf = alloca [3 x i32], align 4
  %a0 = getelementptr inbounds [3 x i32], ptr %buf, i64 0, i64 0
  store i32 16909060, ptr %a0, align 4   ; 0x01020304
  %a1 = getelementptr inbounds [3 x i32], ptr %buf, i64 0, i64 1
  store i32 84281096, ptr %a1, align 4   ; 0x05060708
  %a2 = getelementptr inbounds [3 x i32], ptr %buf, i64 0, i64 2
  store i32 151653132, ptr %a2, align 4  ; 0x090A0B0C
  ; move bytes [0..8) up to [4..12): dst > src, ranges overlap. A naive
  ; byte-at-a-time forward copy would write dst[4] with src[0] then read the
  ; just-clobbered src[4] on the next step; the interpreter must copy through
  ; a temporary so dst ends up {0x01020304, 0x01020304, 0x05060708}.
  call void @llvm.memmove.p0.p0.i64(ptr %a1, ptr %a0, i64 8, i1 false)
  %r0 = load i32, ptr %a0, align 4
  %r1 = load i32, ptr %a1, align 4
  %s = add i32 %r0, %r1
  ret i32 %s
}

declare void @llvm.memmove.p0.p0.i64(ptr nocapture writeonly, ptr nocapture readonly, i64, i1 immarg)
"#;
    match run_ir(ir).expect("memmove overlap IR should run") {
        RuntimeValue::I32(v) => {
            // dst[0..4] untouched (0x01020304), dst[4..8] = copied 0x01020304.
            assert_eq!(v, 16909060 + 16909060, "memmove result")
        }
        other => panic!("expected I32, got {:?}", other),
    }
}

#[test]
fn test_unknown_mem_intrinsic_family_is_rejected() {
    // `llvm.memcpy.inline` is a distinct family the interpreter does not
    // resolve; it must produce an explicit diagnostic, not silently no-op.
    let ir = r#"
define i32 @main() {
entry:
  %d = alloca i32, align 4
  %s = alloca i32, align 4
  store i32 7, ptr %s, align 4
  call void @llvm.memcpy.inline.p0.p0.i64(ptr %d, ptr %s, i64 4, i1 false)
  %r = load i32, ptr %d, align 4
  ret i32 %r
}

declare void @llvm.memcpy.inline.p0.p0.i64(ptr nocapture writeonly, ptr nocapture readonly, i64, i1 immarg)
"#;
    let module = translate_llvm(ir).expect("inline memcpy IR should translate");
    let mut interp = Interpreter::new(module, 65536, 4096);
    let err = interp.run().expect_err("unsupported mem intrinsic must error");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("llvm.memcpy.inline"),
        "diagnostic should name the intrinsic, got: {msg}"
    );
}
