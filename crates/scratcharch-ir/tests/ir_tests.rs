use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::GepIndex;
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::types::IrType;
use scratcharch_core::program::Program;

fn lower_and_get_program(builder: IrBuilder) -> Program {
    let module = builder.finish();
    let lowerer = IsaLowerer::new();
    lowerer.lower(&module).expect("lowering failed")
}

// ── Build & Validation ──────────────────────────────────────────

#[test]
fn test_build_add_function() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("add", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    let s = builder.add(IrType::I32, a, b);
    builder.ret(Some(s));
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let one = builder.const_i32(20);
    let two = builder.const_i32(22);
    let r = builder.call(IrType::I32, "add", vec![one, two]).unwrap();
    builder.ret(Some(r));
    let program = lower_and_get_program(builder);
    assert_eq!(program.functions.len(), 2);
    assert!(program.get_function("add").is_some());
    assert!(program.get_function("main").is_some());
}

#[test]
fn test_validate_entry_block_first() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("second");
    builder.new_block("entry");
    let _ = builder.const_i32(42);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_err());
}

#[test]
fn test_module_validate_functions() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("add", IrType::I32);
    let _a = builder.add_param(IrType::I32, "a");
    let _b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    builder.ret(None);
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let _ = builder.const_i32(0);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

#[test]
fn test_validate_phi_predecessor_check() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let cond = builder.const_i1(true);
    let _a = builder.const_i32(1);
    builder.cond_br(cond, "then", "else_");
    builder.new_block("then");
    let b = builder.const_i32(2);
    builder.br("join");
    builder.new_block("else_");
    let c = builder.const_i32(3);
    builder.br("join");
    builder.new_block("join");
    let _ = builder.phi(IrType::I32, vec![(b, "then"), (c, "else_")]);
    builder.ret(None);
    let module = builder.finish();
    match module.validate() {
        Ok(_) => {}
        Err(e) => panic!("validation failed: {}", e),
    }
}

#[test]
fn test_validate_phi_wrong_predecessor() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(1);
    builder.br("join");
    builder.new_block("join");
    let _ = builder.phi(IrType::I32, vec![(a, "nonexistent")]);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_err());
}

#[test]
fn test_validate_unreachable_block() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let _ = builder.const_i32(42);
    builder.ret(None);
    builder.new_block("unreachable");
    let _ = builder.const_i32(0);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_err());
}

#[test]
fn test_validate_undefined_block_label() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let _ = builder.const_i32(42);
    builder.br("nowhere");
    let module = builder.finish();
    assert!(module.validate().is_err());
}

// ── Type support ─────────────────────────────────────────────────

#[test]
fn test_const_i1() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I1);
    builder.new_block("entry");
    let _t = builder.const_i1(true);
    let _f = builder.const_i1(false);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

#[test]
fn test_const_i8() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I8);
    builder.new_block("entry");
    let _v = builder.const_i8(42);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

#[test]
fn test_const_i16() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I16);
    builder.new_block("entry");
    let _v = builder.const_i16(42000);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

// ── GEP ──────────────────────────────────────────────────────────

#[test]
fn test_gep_build_single_index() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let idx = builder.const_i32(0);
    let _result = builder.gep(IrType::I32, ptr, vec![GepIndex::Dynamic(idx)]);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

#[test]
fn test_gep_struct_field() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let _result = builder.gep(IrType::I32, ptr, vec![GepIndex::StructField(0)]);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

#[test]
fn test_gep_multi_index() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let idx0 = builder.const_i32(0);
    let idx1 = builder.const_i32(2);
    let _result = builder.gep(IrType::I32, ptr, vec![
        GepIndex::Dynamic(idx0),
        GepIndex::Dynamic(idx1),
    ]);
    builder.ret(None);
    let module = builder.finish();
    assert!(module.validate().is_ok());
}

// ── Lowering errors ──────────────────────────────────────────────

#[test]
fn test_lower_cannot_phi() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(1);
    builder.cond_br(a, "then", "else_");
    builder.new_block("then");
    let v42 = builder.const_i32(42);
    builder.br("end");
    builder.new_block("else_");
    let v0 = builder.const_i32(0);
    builder.br("end");
    builder.new_block("end");
    let _result = builder.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    builder.ret(Some(a));
    let module = builder.finish();
    let lowerer = IsaLowerer::new();
    let result = lowerer.lower(&module);
    assert!(result.is_err());
}

#[test]
fn test_lower_conditional() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let cmp = builder.eq(IrType::I32, zero, one);
    builder.cond_br(cmp, "then", "else_");
    builder.new_block("then");
    let v42 = builder.const_i32(42);
    builder.br("end");
    builder.new_block("else_");
    let v0 = builder.const_i32(0);
    builder.br("end");
    builder.new_block("end");
    let result = builder.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    builder.ret(Some(result));
    let module = builder.finish();
    let lowerer = IsaLowerer::new();
    let result = lowerer.lower(&module);
    assert!(result.is_err());
    match result.unwrap_err() {
        scratcharch_ir::lower::LowerError::MultiBlockNotSupported(_) => {}
        _ => panic!("expected MultiBlockNotSupported"),
    }
}

#[test]
fn test_lower_cannot_gep() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let idx = builder.const_i32(0);
    let _result = builder.gep(IrType::I32, ptr, vec![GepIndex::Dynamic(idx)]);
    builder.ret(None);
    let module = builder.finish();
    let lowerer = IsaLowerer::new();
    let result = lowerer.lower(&module);
    assert!(result.is_err());
}

// ── Arithmetic lowering + VM execution ──────────────────────────

#[test]
fn test_lower_arithmetic() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let s = builder.add(IrType::I32, a, b);
    builder.ret(Some(s));
    let program = lower_and_get_program(builder);
    let main = program.get_function("main").unwrap();
    let (_, last) = main.instructions.last().unwrap();
    assert_eq!(last.name(), "return");
}

#[test]
fn test_lower_alloca_store_load() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let val = builder.const_i32(42);
    builder.store(IrType::I32, val, ptr);
    let loaded = builder.load(IrType::I32, ptr);
    builder.ret(Some(loaded));
    let program = lower_and_get_program(builder);
    let main = program.get_function("main").unwrap();
    assert!(main.instructions.len() >= 5);
}

#[test]
fn test_lower_and_run_arithmetic() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let s = builder.add(IrType::I32, a, b);
    builder.ret(Some(s));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_lower_and_run_function_call() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("add", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    let s = builder.add(IrType::I32, a, b);
    builder.ret(Some(s));
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let one = builder.const_i32(20);
    let two = builder.const_i32(22);
    let r = builder.call(IrType::I32, "add", vec![one, two]).unwrap();
    builder.ret(Some(r));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_lower_and_run_subtraction() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(50);
    let b = builder.const_i32(8);
    let s = builder.sub(IrType::I32, a, b);
    builder.ret(Some(s));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_lower_and_run_multiplication() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(6);
    let b = builder.const_i32(7);
    let p = builder.mul(IrType::I32, a, b);
    builder.ret(Some(p));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_lower_and_run_comparison() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I1);
    builder.new_block("entry");
    let five = builder.const_i32(5);
    let five2 = builder.const_i32(5);
    let eq = builder.eq(IrType::I32, five, five2);
    builder.ret(Some(eq));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run_with_limit(100).expect("run failed");
    match vm.stack.peek().unwrap() {
        scratcharch_core::value::Value::I1(v) => assert!(v),
        _ => panic!("expected i1 result"),
    }
}
