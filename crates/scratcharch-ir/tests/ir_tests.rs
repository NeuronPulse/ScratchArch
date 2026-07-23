use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::GepIndex;
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::types::IrType;
use scratcharch_core::program::Program;
use scratcharch_target::profile::{TargetProfile, Endianness, IntegerModel, MemoryModel, AbiVersion};

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
fn test_lower_diamond_phi() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i1(true);
    builder.cond_br(a, "then", "else_");
    builder.new_block("then");
    let v42 = builder.const_i32(42);
    builder.br("end");
    builder.new_block("else_");
    let v0 = builder.const_i32(0);
    builder.br("end");
    builder.new_block("end");
    let result = builder.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    builder.ret(Some(result));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_lower_conditional_false_phi() {
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
    let v0 = builder.const_i32(7);
    builder.br("end");
    builder.new_block("end");
    let result = builder.phi(IrType::I32, vec![(v42, "then"), (v0, "else_")]);
    builder.ret(Some(result));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(7));
}

#[test]
fn test_lower_gep_array() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let arr = builder.alloca_array(IrType::I32, 4);
    let idx = builder.const_i32(2);
    let elem_ptr = builder.gep(IrType::I32, arr, vec![GepIndex::Dynamic(idx)]);
    let val = builder.const_i32(99);
    builder.store(IrType::I32, val, elem_ptr);
    let loaded = builder.load(IrType::I32, elem_ptr);
    builder.ret(Some(loaded));
    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(99));
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

// ── Multi-block / phi lowering tests ─────────────────────────────

#[test]
fn test_lower_loop_phi() {
    // sum = 0; for i = 0; i < 5; i++ { sum += i }; return sum (== 10)
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let five = builder.const_i32(5);
    builder.br("check");

    builder.new_block("check");
    // Phis are the first two instructions in this block. Their back-edge
    // operands are patched once the loop-body values have been defined.
    let i = builder.phi(IrType::I32, vec![(zero, "entry"), (zero, "body")]);
    let acc = builder.phi(IrType::I32, vec![(zero, "entry"), (zero, "body")]);
    let cond = builder.lt(IrType::I32, i, five);
    builder.cond_br(cond, "body", "end");

    builder.new_block("body");
    let acc_next = builder.add(IrType::I32, acc, i);
    let i_next = builder.add(IrType::I32, i, one);
    builder.set_phi_operand("check", 0, "body", i_next);
    builder.set_phi_operand("check", 1, "body", acc_next);
    builder.br("check");

    builder.new_block("end");
    builder.ret(Some(acc));

    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(10));
}

#[test]
fn test_lower_nested_branches() {
    // if (a) { if (b) { 1 } else { 2 } } else { 3 }
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i1(true);
    let b = builder.const_i1(false);
    builder.cond_br(a, "outer_then", "outer_else");

    builder.new_block("outer_then");
    builder.cond_br(b, "inner_then", "inner_else");

    builder.new_block("inner_then");
    let v1 = builder.const_i32(1);
    builder.br("inner_join");

    builder.new_block("inner_else");
    let v2 = builder.const_i32(2);
    builder.br("inner_join");

    builder.new_block("inner_join");
    let inner_phi = builder.phi(IrType::I32, vec![(v1, "inner_then"), (v2, "inner_else")]);
    builder.br("outer_join");

    builder.new_block("outer_else");
    let v3 = builder.const_i32(3);
    builder.br("outer_join");

    builder.new_block("outer_join");
    let result = builder.phi(
        IrType::I32,
        vec![(inner_phi, "inner_join"), (v3, "outer_else")],
    );
    builder.ret(Some(result));

    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(2));
}

#[test]
fn test_lower_and_run_factorial() {
    let mut builder = IrBuilder::new("main");

    builder.start_function("fact", IrType::I32);
    let n = builder.add_param(IrType::I32, "n");
    builder.new_block("entry");
    let one = builder.const_i32(1);
    let two = builder.const_i32(2);
    let cond = builder.lt(IrType::I32, n, two);
    builder.cond_br(cond, "base", "recur");

    builder.new_block("base");
    builder.ret(Some(one));

    builder.new_block("recur");
    let nm1 = builder.sub(IrType::I32, n, one);
    let rec = builder.call(IrType::I32, "fact", vec![nm1]).unwrap();
    let result = builder.mul(IrType::I32, n, rec);
    builder.ret(Some(result));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let five = builder.const_i32(5);
    let r = builder.call(IrType::I32, "fact", vec![five]).unwrap();
    builder.ret(Some(r));

    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(120));
}

#[test]
fn test_lower_and_run_fib() {
    // fib(0) = 0, fib(1) = 1, fib(n) = fib(n-1) + fib(n-2)
    let mut builder = IrBuilder::new("main");

    builder.start_function("fib", IrType::I32);
    let n = builder.add_param(IrType::I32, "n");
    builder.new_block("entry");
    let _zero = builder.const_i32(0);
    let one = builder.const_i32(1);
    let two = builder.const_i32(2);
    let cond = builder.lt(IrType::I32, n, two);
    builder.cond_br(cond, "base", "recur");

    builder.new_block("base");
    // fib(0) = 0, fib(1) = 1; for n < 2 the parameter value itself is correct.
    builder.ret(Some(n));

    builder.new_block("recur");
    let nm1 = builder.sub(IrType::I32, n, one);
    let nm2 = builder.sub(IrType::I32, n, two);
    let a = builder.call(IrType::I32, "fib", vec![nm1]).unwrap();
    let b = builder.call(IrType::I32, "fib", vec![nm2]).unwrap();
    let result = builder.add(IrType::I32, a, b);
    builder.ret(Some(result));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ten = builder.const_i32(10);
    let r = builder.call(IrType::I32, "fib", vec![ten]).unwrap();
    builder.ret(Some(r));

    let program = lower_and_get_program(builder);
    let mut vm = scratcharch_vm::vm::Vm::new(65536, 4096);
    vm.load_program(&program).expect("load failed");
    vm.run().expect("run failed");
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(55));
}

#[test]
fn test_target_aware_slot_allocation() {
    let profile = TargetProfile {
        name: "sa16",
        cell_width: 16,
        pointer_width: 32,
        endianness: Endianness::Little,
        integer_model: IntegerModel::ModularWrapping,
        memory_model: MemoryModel::FlatByteAddressable,
        abi_version: AbiVersion::V0_1,
    };

    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    let _ = builder.add_param(IrType::I32, "a");
    builder.new_block("entry");
    builder.ret(None);

    let module = builder.finish();
    let lowerer = IsaLowerer::with_profile(profile);
    let program = lowerer.lower(&module).expect("lowering failed");
    let main = program.get_function("main").unwrap();
    // i32 is 32 bits; sa16 has 16-bit cells, so i32 occupies 2 cells.
    assert_eq!(main.param_cells, 2);
    assert_eq!(main.return_cells, 2);
}
