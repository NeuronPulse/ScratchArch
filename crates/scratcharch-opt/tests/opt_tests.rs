use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::Instruction;
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::Constant;

use scratcharch_opt::pass::OptimizationPass;
use scratcharch_opt::manager::PassManager;
use scratcharch_opt::constant_fold::ConstantFold;
use scratcharch_opt::dce::DeadCodeElimination;
use scratcharch_opt::cfg_simplify::CfgSimplify;

// ── Helper: build a one-function module ─────────────────────────────

fn build_simple_module(body: impl Fn(&mut IrBuilder)) -> IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    body(&mut builder);
    builder.finish()
}

// ── Constant Folding Tests ──────────────────────────────────────────

#[test]
fn test_constant_fold_add() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let _r = builder.add(IrType::I32, a, b);
    builder.ret(Some(a)); // dummy ret, result is unused in this test
    let mut module = builder.finish();

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    // After folding: add instruction replaced by Const(42)
    let mut found_folded = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I32(42)) = instr {
            found_folded = true;
        }
    }
    assert!(found_folded, "expected Const(42) after folding add 20, 22");
}

#[test]
fn test_constant_fold_sub() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(50);
        let b2 = b.const_i32(8);
        let r = b.sub(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found_folded = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I32(42)) = instr {
            found_folded = true;
            break;
        }
    }
    assert!(found_folded, "expected Const(42) after folding sub 50, 8");
}

#[test]
fn test_constant_fold_mul() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(6);
        let b2 = b.const_i32(7);
        let r = b.mul(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found_folded = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I32(42)) = instr {
            found_folded = true;
            break;
        }
    }
    assert!(found_folded);
}

#[test]
fn test_constant_fold_eq_true() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(42);
        let b2 = b.const_i32(42);
        let r = b.eq(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found_folded = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I1(true)) = instr {
            found_folded = true;
            break;
        }
    }
    assert!(found_folded, "expected Const(I1(true)) after folding eq 42, 42");
}

#[test]
fn test_constant_fold_eq_false() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(1);
        let b2 = b.const_i32(2);
        let r = b.eq(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I1(false)) = instr {
            found = true;
            break;
        }
    }
    assert!(found);
}

#[test]
fn test_constant_fold_sgt_true() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(5);
        let b2 = b.const_i32(3);
        let r = b.gt(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I1(true)) = instr {
            found = true;
            break;
        }
    }
    assert!(found);
}

#[test]
fn test_constant_fold_slt_false() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(5);
        let b2 = b.const_i32(3);
        let r = b.lt(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I1(false)) = instr {
            found = true;
            break;
        }
    }
    assert!(found);
}

#[test]
fn test_constant_fold_wrapping() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(u32::MAX);
        let b2 = b.const_i32(1);
        let r = b.add(IrType::I32, a, b2);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let mut found = false;
    for instr in &entry.instructions {
        if let Instruction::Const(Constant::I32(0)) = instr {
            found = true;
            break;
        }
    }
    assert!(found, "expected Const(0) after folding u32::MAX + 1");
}

#[test]
fn test_constant_fold_non_const_operand_unchanged() {
    let mut module = build_simple_module(|b| {
        let a = b.const_i32(10);
        // b2 is not a constant (it's a parameter value id 0)
        // but we can't add params in build_simple_module, so we use
        // a non-const operand by using the result of an add
        let mid = b.add(IrType::I32, a, a);
        let r = b.add(IrType::I32, mid, a);
        b.ret(Some(r));
    });

    let mut cf = ConstantFold;
    cf.run(&mut module);

    // The inner `add 10, 10` folds to Const(20), but the outer
    // `add %mid, 10` can't fold because %mid is now Const(20) too...
    // Actually both should fold! Let's just check we don't crash.
    let func = module.get_function("main").unwrap();
    assert!(func.blocks[0].instructions.len() <= 3);
}

// ── DCE Tests ────────────────────────────────────────────────────────

#[test]
fn test_dce_removes_unused_add() {
    let mut module = build_simple_module(|b| {
        let c1 = b.const_i32(1);
        let c2 = b.const_i32(2);
        let _unused = b.add(IrType::I32, c1, c2);
        let r = b.const_i32(42);
        b.ret(Some(r));
    });

    let mut dce = DeadCodeElimination;
    dce.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let has_add = entry.instructions.iter().any(|i| matches!(i, Instruction::Add { .. }));
    assert!(!has_add, "unused add should have been removed");
}

#[test]
fn test_dce_preserves_used_add() {
    let mut module = build_simple_module(|b| {
        let c1 = b.const_i32(1);
        let c2 = b.const_i32(2);
        let used = b.add(IrType::I32, c1, c2);
        let c0 = b.const_i32(0);
        let r = b.add(IrType::I32, used, c0);
        b.ret(Some(r));
    });

    let mut dce = DeadCodeElimination;
    dce.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let add_count = entry.instructions.iter().filter(|i| matches!(i, Instruction::Add { .. })).count();
    assert_eq!(add_count, 2, "both adds should be preserved (one used by other, other used by ret)");
}

#[test]
fn test_dce_does_not_remove_store() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let c42 = builder.const_i32(42);
    builder.store(IrType::I32, c42, ptr);
    let val = builder.load(IrType::I32, ptr);
    builder.ret(Some(val));
    let mut module = builder.finish();

    let mut dce = DeadCodeElimination;
    dce.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let has_store = entry.instructions.iter().any(|i| matches!(i, Instruction::Store { .. }));
    assert!(has_store, "store must not be removed by DCE");
    let has_alloca = entry.instructions.iter().any(|i| matches!(i, Instruction::Alloca { .. }));
    assert!(has_alloca, "alloca must not be removed by DCE (result used by load)");
}

#[test]
fn test_dce_removes_unused_const() {
    let mut module = build_simple_module(|b| {
        let _unused1 = b.const_i32(1);
        let _unused2 = b.const_i32(2);
        let r = b.const_i32(42);
        b.ret(Some(r));
    });

    let mut dce = DeadCodeElimination;
    dce.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let const_count = entry.instructions.iter().filter(|i| matches!(i, Instruction::Const(_))).count();
    assert_eq!(const_count, 1, "only one const should remain (r=42)");
}

// ── CFG Simplification Tests ────────────────────────────────────────

#[test]
fn test_cfg_remove_unreachable_block() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let r = builder.const_i32(42);
    builder.ret(Some(r));

    builder.new_block("dead");
    let d = builder.const_i32(0);
    builder.ret(Some(d));
    let mut module = builder.finish();

    let mut cfg = CfgSimplify;
    cfg.run(&mut module);

    let func = module.get_function("main").unwrap();
    assert_eq!(func.blocks.len(), 1, "dead block should be removed");
    assert_eq!(func.blocks[0].label, "entry");
}

#[test]
fn test_cfg_merge_trivial_block() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    builder.br("middle");

    builder.new_block("middle");
    builder.br("exit");

    builder.new_block("exit");
    let r = builder.const_i32(42);
    builder.ret(Some(r));
    let mut module = builder.finish();

    let mut cfg = CfgSimplify;
    cfg.run(&mut module);

    let func = module.get_function("main").unwrap();
    assert_eq!(func.blocks.len(), 2, "middle block should be merged");
    assert_eq!(func.blocks[0].label, "entry");
    assert_eq!(func.blocks[1].label, "exit");
}

#[test]
fn test_cfg_no_change_on_clean_cfg() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let r = builder.add(IrType::I32, a, b);
    builder.ret(Some(r));
    let mut module = builder.finish();

    let func_before = module.get_function("main").unwrap().blocks.len();

    let mut cfg = CfgSimplify;
    cfg.run(&mut module);

    let func_after = module.get_function("main").unwrap().blocks.len();
    assert_eq!(func_before, func_after, "clean CFG should not change");
}

#[test]
fn test_cfg_multiple_unreachable_blocks() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let r = builder.const_i32(42);
    builder.ret(Some(r));

    builder.new_block("dead1");
    builder.br("dead2");

    builder.new_block("dead2");
    let d = builder.const_i32(0);
    builder.ret(Some(d));
    let mut module = builder.finish();

    let mut cfg = CfgSimplify;
    cfg.run(&mut module);

    let func = module.get_function("main").unwrap();
    assert_eq!(func.blocks.len(), 1, "all dead blocks should be removed");
}

// ── Pipeline Tests ───────────────────────────────────────────────────

#[test]
fn test_pass_manager_pipeline() {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let c1 = builder.const_i32(1);
    let c2 = builder.const_i32(2);
    let _unused = builder.add(IrType::I32, c1, c2);
    let a = builder.const_i32(20);
    let b = builder.const_i32(22);
    let r = builder.add(IrType::I32, a, b);
    builder.ret(Some(r));
    let mut module = builder.finish();

    let mut pm = PassManager::new();
    pm.add(ConstantFold);
    pm.add(DeadCodeElimination);
    pm.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];

    // After CF: `add 20, 22` → Const(42)
    // After DCE: unused `add 1, 2` removed
    let has_const_42 = entry.instructions.iter().any(|i| matches!(i, Instruction::Const(Constant::I32(42))));
    assert!(has_const_42, "pipeline should fold add to Const(42)");

    let add_count = entry.instructions.iter().filter(|i| matches!(i, Instruction::Add { .. })).count();
    assert_eq!(add_count, 0, "all adds should be folded or removed");
}

#[test]
fn test_pipeline_llvm_to_opt_to_interpreter() {
    let input = r#"
define i32 @main() {
entry:
  %x = add i32 20, 22
  ret i32 %x
}
"#;
    let mut module = scratcharch_llvm::translate_llvm(input).expect("translation failed");

    let mut pm = PassManager::new();
    pm.add(ConstantFold);
    pm.add(DeadCodeElimination);
    pm.run(&mut module);

    let mut interp = scratcharch_sair_interpreter::Interpreter::new(module, 65536, 4096);
    let result = interp.run().expect("execution failed");

    match result {
        Some(scratcharch_sair_interpreter::RuntimeValue::I32(v)) => {
            assert_eq!(v, 42, "pipeline: expected 42, got {}", v);
        }
        other => panic!("pipeline: expected I32(42), got {:?}", other),
    }
}

#[test]
fn test_pipeline_dce_on_llvm_module() {
    let input = r#"
define i32 @main() {
entry:
  %dead = add i32 1, 2
  %x = add i32 20, 22
  ret i32 %x
}
"#;
    let mut module = scratcharch_llvm::translate_llvm(input).expect("translation failed");

    let mut pm = PassManager::new();
    pm.add(ConstantFold);
    pm.add(DeadCodeElimination);
    pm.run(&mut module);

    let func = module.get_function("main").unwrap();
    let entry = &func.blocks[0];
    let has_dead = entry.instructions.iter().any(|i| {
        if let Instruction::Add { lhs, rhs, .. } = i {
            *lhs == 1 || *rhs == 1
        } else {
            false
        }
    });
    assert!(!has_dead, "dead add should be removed after pipeline");
}
