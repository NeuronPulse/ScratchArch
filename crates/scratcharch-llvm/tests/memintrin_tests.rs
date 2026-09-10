//! LLVM memory-intrinsic tests (`llvm.memcpy`/`llvm.memmove`/`llvm.memset`).
//!
//! Part 4 of the v0.2 LLVM compatibility work: real clang emits these from the
//! standard library `memcpy`/`memmove`/`memset` builtins. Constant-length,
//! non-volatile calls to the *canonical* families (`llvm.memcpy.p0.p0.iN`,
//! `llvm.memmove.p0.p0.iN`, `llvm.memset.p0.iN`, and the three-operand
//! `__scratcharch_memcpy` runtime helper) are expanded by the translator into
//! width-exact `i8` load/store sequences, so the same program runs on the ISA VM
//! as well as the interpreter. The copy expansion reads every source byte before
//! storing any destination byte (as-if-through-a-temporary), which makes
//! `memmove` well-defined on overlapping regions.
//!
//! Everything else — non-canonical variants (`llvm.memcpy.inline.*`,
//! `llvm.memcpy.element.unordered.*`), runtime-length, volatile, or oversized
//! calls — is left as an ordinary call: the interpreter resolves a canonical
//! family against its flat memory at run time and the VM resolves it at load
//! time (its runtime resolver, `scratcharch-vm/src/runtime.rs`), while a
//! non-canonical variant is rejected by both with a named diagnostic. A callee
//! neither engine knows is an explicit rejection, never a silent approximation.

use std::path::PathBuf;

use scratcharch_ir::lower::IsaLowerer;
use scratcharch_llvm::translate_llvm;
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_sair_interpreter::RuntimeValue;
use scratcharch_vm::vm::Vm;

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/scratcharch-llvm
    p.pop(); // project root
    p
}

/// Translate an LLVM module and run it on the SAIR interpreter; the returned
/// `Option` is `None` for a void `main`, else the produced value.
fn run_interp(ir: &str) -> Result<Option<RuntimeValue>, String> {
    let module = translate_llvm(ir).map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    interp
        .run()
        .map_err(|e| format!("interpreter: {:?}", e))
}

/// Translate an LLVM module, lower it to the ISA, and run it on the VM. The
/// returned stack cell is the `i32` value `main` produced.
fn run_vm(ir: &str) -> Result<u32, String> {
    let module = translate_llvm(ir).map_err(|e| e.to_string())?;
    let program = IsaLowerer::new()
        .lower(&module)
        .map_err(|e| format!("lower: {e:?}"))?;
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&program).map_err(|e| e.to_string())?;
    vm.run().map_err(|e| e.to_string())?;
    let top = vm
        .stack
        .peek()
        .map_err(|e| format!("VM stack: {e:?}"))?;
    match top.as_i32() {
        Some(v) => Ok(v),
        None => Err(format!("VM: expected I32 on the operand stack, got {top:?}")),
    }
}

/// Translate + run on both engines and require bit agreement, returning the
/// interpreter's value.
fn assert_interp_vm_agree(ir: &str) -> u32 {
    let interp = match run_interp(ir).expect("interpreter should run") {
        Some(RuntimeValue::I32(v)) => v,
        other => panic!("interpreter expected I32, got {other:?}"),
    };
    let vm = run_vm(ir).expect("VM should run the expanded module");
    assert_eq!(vm, interp, "interpreter {interp} disagrees with VM {vm}");
    interp
}

/// Translate an LLVM module with a single `i32` result and run it on the
/// interpreter (used for the committed fixture).
fn run_fixture(name: &str) -> Result<RuntimeValue, String> {
    let path = project_root().join("tests").join("c_programs").join(format!("{name}.ll"));
    let ir = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match run_interp(&ir)? {
        Some(v) => Ok(v),
        None => Err("fixture main returned nothing".to_string()),
    }
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
fn test_memmove_reverse_overlap_is_well_defined_on_both_engines() {
    // memmove where dst < src with overlapping regions: bytes must not be
    // clobbered by a forward byte copy. Write a 12-byte buffer via three i32
    // GEP stores, move the middle 4 bytes down by 4, and read them back. The
    // translator expands the call into a load-all-then-store sequence, so the
    // as-if-through-a-temporary semantics must hold on the VM too.
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
  ; just-clobbered src[4] on the next step; the load-all expansion copies
  ; through a temporary so dst ends up {0x01020304, 0x01020304, 0x05060708}.
  call void @llvm.memmove.p0.p0.i64(ptr %a1, ptr %a0, i64 8, i1 false)
  %r0 = load i32, ptr %a0, align 4
  %r1 = load i32, ptr %a1, align 4
  %s = add i32 %r0, %r1
  ret i32 %s
}

declare void @llvm.memmove.p0.p0.i64(ptr nocapture writeonly, ptr nocapture readonly, i64, i1 immarg)
"#;
    // dst[0..4] untouched (0x01020304), dst[4..8] = copied 0x01020304.
    assert_eq!(assert_interp_vm_agree(ir), 16909060 + 16909060, "memmove result");
}

#[test]
fn test_constant_length_memcpy_and_memset_expand_to_vm() {
    // A constant-length memcpy of four bytes {0x78 0x56 0x34 0x12} (little-
    // endian → 0x12345678) and a constant-length memset of a whole buffer to
    // 0xAB. Both calls are expanded by the translator, so interpreter and VM
    // must agree on the combined result.
    let ir = r#"
define i32 @main() {
entry:
  %dst = alloca [4 x i8], align 1
  %src = alloca [4 x i8], align 1
  store i8 120, ptr %src, align 1  ; 0x78
  %s1 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 1
  store i8 86, ptr %s1, align 1    ; 0x56
  %s2 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 2
  store i8 52, ptr %s2, align 1    ; 0x34
  %s3 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 3
  store i8 18, ptr %s3, align 1    ; 0x12
  call void @llvm.memcpy.p0.p0.i32(ptr %dst, ptr %src, i32 4, i1 false)
  %r = load i32, ptr %dst, align 1 ; 0x12345678
  %m = alloca [4 x i8], align 1
  call void @llvm.memset.p0.i32(ptr %m, i8 -85, i32 4, i1 false) ; 0xAB
  %m0 = load i8, ptr %m, align 1
  %mu = zext i8 %m0 to i32
  %s = add i32 %r, %mu
  ret i32 %s
}

declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
"#;
    assert_eq!(assert_interp_vm_agree(ir), 0x12345678u32 + 0xAB, "copy+set result");
}

#[test]
fn test_unknown_mem_intrinsic_family_is_rejected() {
    // `llvm.memcpy.inline` is a *distinct family* that merely shares the
    // `llvm.memcpy.` prefix; it is not canonical (its variant is not
    // `p<d>.p<s>.i<N>`), so the translator leaves it as a call and the
    // interpreter must reject it with an explicit diagnostic, not silently
    // no-op — and never expand it as if it were a plain memcpy.
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

#[test]
fn test_runtime_length_memcpy_runs_on_both_engines() {
    // A length read from memory is not a compile-time constant, so the
    // translator leaves the call intact. Both engines resolve the canonical
    // `llvm.memcpy` at the runtime length: the interpreter against its flat
    // memory at run time, the VM at load time (its runtime resolver rewrites
    // the bodyless named callee), so they must agree exactly.
    let ir = r#"
define i32 @main() {
entry:
  %dst = alloca [4 x i8], align 1
  %src = alloca [4 x i8], align 1
  %lenp = alloca i64, align 8
  store i64 4, ptr %lenp, align 8
  store i8 1, ptr %src, align 1
  %s1 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 1
  store i8 2, ptr %s1, align 1
  %s2 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 2
  store i8 3, ptr %s2, align 1
  %s3 = getelementptr inbounds [4 x i8], ptr %src, i64 0, i64 3
  store i8 4, ptr %s3, align 1
  %len = load i64, ptr %lenp, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 %len, i1 false)
  %r = load i32, ptr %dst, align 1
  ret i32 %r
}

declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
"#;

    let module = translate_llvm(ir).expect("runtime-length IR should translate");
    let mut interp = Interpreter::new(module, 65536, 4096);
    let expected = match interp
        .run()
        .expect("interpreter must run the runtime-length memcpy")
    {
        Some(RuntimeValue::I32(v)) => v,
        other => panic!("expected I32, got {other:?}"),
    };
    assert_eq!(expected, 0x04030201, "runtime-length memcpy result");

    let vm_value = run_vm(ir).expect("VM must resolve the runtime-length memcpy");
    assert_eq!(
        vm_value, expected,
        "VM must agree with the interpreter at the runtime length"
    );
}

#[test]
fn test_unknown_runtime_callee_is_rejected_by_both_engines() {
    // A name the runtime registry does not know is never approximated: the
    // interpreter reports it as an unknown intrinsic when the call is reached,
    // and the VM rejects it at *load* time (a bodyless callee it cannot
    // resolve). This is the negative twin of the resolver tests above.
    let ir = r#"
define i32 @main() {
entry:
  %r = call i32 @no_such_runtime_function()
  ret i32 %r
}

declare i32 @no_such_runtime_function()
"#;

    let interp_err = run_interp(ir).expect_err("interpreter must reject the unknown callee");
    assert!(
        interp_err.contains("no_such_runtime_function"),
        "interpreter diagnostic should name the callee, got: {interp_err}"
    );

    let vm_err = run_vm(ir).expect_err("VM must reject the unknown callee");
    assert!(
        vm_err.contains("undefined function: no_such_runtime_function"),
        "VM diagnostic should name the callee, got: {vm_err}"
    );
}
