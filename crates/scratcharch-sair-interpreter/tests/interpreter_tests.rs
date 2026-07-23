use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::GepIndex;
use scratcharch_ir::types::IrType;
use scratcharch_sair_interpreter::Interpreter;

fn run_sair(builder: IrBuilder) -> Result<String, String> {
    let module = builder.finish();
    module.validate().map_err(|e| e.to_string())?;
    let mut interp = Interpreter::new(module, 65536, 4096);
    let result = interp.run().map_err(|e| format!("{:?}", e))?;
    Ok(match result {
        Some(v) => v.to_string(),
        None => "void".to_string(),
    })
}

// ── Basic arithmetic ─────────────────────────────────────────────

#[test]
fn test_interp_add() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(20);
    let bv = b.const_i32(22);
    let s = b.add(IrType::I32, a, bv);
    b.ret(Some(s));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

#[test]
fn test_interp_sub() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(50);
    let bv = b.const_i32(8);
    let s = b.sub(IrType::I32, a, bv);
    b.ret(Some(s));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

#[test]
fn test_interp_mul() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(6);
    let bv = b.const_i32(7);
    let p = b.mul(IrType::I32, a, bv);
    b.ret(Some(p));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

#[test]
fn test_interp_div() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(84);
    let bv = b.const_i32(2);
    let q = b.div(IrType::I32, a, bv);
    b.ret(Some(q));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

#[test]
fn test_interp_rem() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(43);
    let bv = b.const_i32(2);
    let r = b.rem(IrType::I32, a, bv);
    b.ret(Some(r));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 1");
}

// ── Comparison ───────────────────────────────────────────────────

#[test]
fn test_interp_eq() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I1);
    b.new_block("entry");
    let a = b.const_i32(5);
    let bv = b.const_i32(5);
    let e = b.eq(IrType::I32, a, bv);
    b.ret(Some(e));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i1 1");
}

#[test]
fn test_interp_neq() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I1);
    b.new_block("entry");
    let a = b.const_i32(5);
    let bv = b.const_i32(6);
    let e = b.eq(IrType::I32, a, bv);
    b.ret(Some(e));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i1 0");
}

#[test]
fn test_interp_lt() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I1);
    b.new_block("entry");
    let a = b.const_i32(3);
    let bv = b.const_i32(5);
    let lt = b.lt(IrType::I32, a, bv);
    b.ret(Some(lt));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i1 1");
}

#[test]
fn test_interp_gt() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I1);
    b.new_block("entry");
    let a = b.const_i32(7);
    let bv = b.const_i32(5);
    let gt = b.gt(IrType::I32, a, bv);
    b.ret(Some(gt));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i1 1");
}

// ── Memory operations ────────────────────────────────────────────

#[test]
fn test_interp_alloca_store_load() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let ptr = b.alloca(IrType::I32);
    let val = b.const_i32(42);
    b.store(IrType::I32, val, ptr);
    let loaded = b.load(IrType::I32, ptr);
    b.ret(Some(loaded));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

// ── Constants ────────────────────────────────────────────────────

#[test]
fn test_interp_const_i1() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I1);
    b.new_block("entry");
    let v = b.const_i1(true);
    b.ret(Some(v));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i1 1");
}

#[test]
fn test_interp_const_i8() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I8);
    b.new_block("entry");
    let v = b.const_i8(42);
    b.ret(Some(v));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i8 42");
}

#[test]
fn test_interp_const_i16() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I16);
    b.new_block("entry");
    let v = b.const_i16(1000);
    b.ret(Some(v));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i16 1000");
}

// ── Control flow (multi-block) ───────────────────────────────────

#[test]
fn test_interp_conditional_branch() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let cond = b.const_i1(true);
    b.cond_br(cond, "then", "else_");
    b.new_block("then");
    let v42 = b.const_i32(42);
    b.br("end");
    b.new_block("else_");
    let v0 = b.const_i32(0);
    b.br("end");
    b.new_block("end");
    let result = b.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

#[test]
fn test_interp_conditional_branch_false() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let cond = b.const_i1(false);
    b.cond_br(cond, "then", "else_");
    b.new_block("then");
    let v42 = b.const_i32(42);
    b.br("end");
    b.new_block("else_");
    let v0 = b.const_i32(0);
    b.br("end");
    b.new_block("end");
    let result = b.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 0");
}

#[test]
fn test_interp_unconditional_branch() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let a = b.const_i32(10);
    b.br("next");
    b.new_block("next");
    let bv = b.const_i32(32);
    let s = b.add(IrType::I32, a, bv);
    b.ret(Some(s));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

// ── Function calls ───────────────────────────────────────────────

#[test]
fn test_interp_function_call() {
    let mut b = IrBuilder::new("main");
    b.start_function("add", IrType::I32);
    let a = b.add_param(IrType::I32, "a");
    let bv = b.add_param(IrType::I32, "b");
    b.new_block("entry");
    let s = b.add(IrType::I32, a, bv);
    b.ret(Some(s));
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let one = b.const_i32(20);
    let two = b.const_i32(22);
    let r = b.call(IrType::I32, "add", vec![one, two]).unwrap();
    b.ret(Some(r));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

// ── GEP ──────────────────────────────────────────────────────────

#[test]
fn test_interp_gep_single() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::Pointer);
    b.new_block("entry");
    let base = b.alloca(IrType::I32);
    let idx = b.const_i32(2);
    let result = b.gep(IrType::I32, base, vec![GepIndex::Dynamic(idx)]);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    assert!(result.starts_with("ptr "));
}

#[test]
fn test_interp_gep_struct_field() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::Pointer);
    b.new_block("entry");
    let base = b.alloca(IrType::I32);
    let result = b.gep(IrType::I32, base, vec![GepIndex::StructField(1)]);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    assert!(result.starts_with("ptr "));
}

#[test]
fn test_interp_gep_and_access() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    // Allocate array space: 4 * i32 = 16 bytes
    let arr = b.alloca(IrType::I32);
    // Store 42 at arr[0]
    let val = b.const_i32(42);
    b.store(IrType::I32, val, arr);
    // Load from arr[0] using GEP
    let zero = b.const_i32(0);
    let elem_ptr = b.gep(IrType::I32, arr, vec![GepIndex::Dynamic(zero)]);
    let loaded = b.load(IrType::I32, elem_ptr);
    b.ret(Some(loaded));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 42");
}

// ── Multi-block with phi ─────────────────────────────────────────

#[test]
fn test_interp_phi_select() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let cond = b.const_i1(true);
    let a = b.const_i32(100);
    let bv = b.const_i32(200);
    b.cond_br(cond, "then", "else_");
    b.new_block("then");
    b.br("join");
    b.new_block("else_");
    b.br("join");
    b.new_block("join");
    let result = b.phi(IrType::I32, vec![(a, "then"), (bv, "else_")]);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    // Values from "then" block: a = 100
    // Values from "else_" block: bv = 200
    // Since cond is true, we get "then" block's value = 100
    assert_eq!(result, "i32 100");
}

#[test]
fn test_interp_void_function() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::Void);
    b.new_block("entry");
    let _ = b.const_i32(42);
    b.ret(None);
    let result = run_sair(b).unwrap();
    assert_eq!(result, "void");
}

#[test]
fn test_interp_wrapping_arithmetic() {
    let mut b = IrBuilder::new("main");
    b.start_function("main", IrType::I32);
    b.new_block("entry");
    let max = b.const_i32(u32::MAX);
    let one = b.const_i32(1);
    let result = b.add(IrType::I32, max, one);
    b.ret(Some(result));
    let result = run_sair(b).unwrap();
    assert_eq!(result, "i32 0");
}
