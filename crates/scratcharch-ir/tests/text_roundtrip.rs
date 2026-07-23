use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::debug::DebugLoc;
use scratcharch_ir::instruction::GepIndex;
use scratcharch_ir::types::IrType;
use scratcharch_ir::text;

fn simple_module() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let c = builder.const_i32(42);
    builder.ret(Some(c));
    builder.finish()
}

fn module_with_params() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("add");
    builder.start_function("add", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    let sum = builder.add(IrType::I32, a, b);
    builder.ret(Some(sum));
    builder.finish()
}

fn module_with_control_flow() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("max");
    builder.start_function("max", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");

    builder.new_block("entry");
    let cond = builder.gt(IrType::I32, a, b);
    builder.cond_br(cond, "then", "else");

    builder.new_block("then");
    builder.br("merge");

    builder.new_block("else");
    builder.br("merge");

    builder.new_block("merge");
    let phi = builder.phi(IrType::I32, vec![(a, "then"), (b, "else")]);
    builder.ret(Some(phi));

    builder.finish()
}

fn module_with_memory_and_call() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let ptr = builder.alloca(IrType::I32);
    let c = builder.const_i32(7);
    builder.store(IrType::I32, c, ptr);
    let v = builder.load(IrType::I32, ptr);
    let one = builder.const_i32(1);
    let r = builder.add(IrType::I32, v, one);
    builder.ret(Some(r));
    builder.finish()
}

fn module_with_gep() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let arr = builder.alloca_array(IrType::I32, 4);
    let idx = builder.const_i32(2);
    let elem = builder.gep(IrType::I32, arr, vec![GepIndex::Dynamic(idx)]);
    let val = builder.const_i32(99);
    builder.store(IrType::I32, val, elem);
    let loaded = builder.load(IrType::I32, elem);
    builder.ret(Some(loaded));
    builder.finish()
}

fn module_with_debug_locs() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let c = builder.const_i32(42);
    builder.set_last_debug_loc(Some(DebugLoc::new("src.c", 10, Some(5))));
    builder.ret(Some(c));
    builder.finish()
}

fn module_with_calls() -> scratcharch_ir::r#module::IrModule {
    let mut builder = IrBuilder::new("main");
    builder.start_function("add", IrType::I32);
    let a = builder.add_param(IrType::I32, "a");
    let b = builder.add_param(IrType::I32, "b");
    builder.new_block("entry");
    let sum = builder.add(IrType::I32, a, b);
    builder.ret(Some(sum));

    builder.start_function("main", IrType::I32);
    builder.new_block("entry");
    let one = builder.const_i32(1);
    let two = builder.const_i32(2);
    let three = builder.const_i32(3);
    let r = builder.call(IrType::I32, "add", vec![one, two]);
    builder.call(IrType::Void, "dummy", vec![three]);
    builder.ret(r);
    builder.finish()
}

fn roundtrip(module: &scratcharch_ir::r#module::IrModule) {
    module.validate().expect("module should be valid");
    let text = text::serialize(module);
    let reparsed = text::deserialize(&text).expect("deserialize should succeed");
    let text2 = text::serialize(&reparsed);
    assert_eq!(
        text, text2,
        "round-trip produced different text:\n--- original ---\n{}\n--- reparsed ---\n{}",
        text, text2
    );
}

#[test]
fn roundtrip_simple() {
    roundtrip(&simple_module());
}

#[test]
fn roundtrip_params() {
    roundtrip(&module_with_params());
}

#[test]
fn roundtrip_control_flow() {
    roundtrip(&module_with_control_flow());
}

#[test]
fn roundtrip_memory_and_call() {
    roundtrip(&module_with_memory_and_call());
}

#[test]
fn roundtrip_gep() {
    roundtrip(&module_with_gep());
}

#[test]
fn serialize_deserialize_simple() {
    let module = simple_module();
    let text = text::serialize(&module);
    let reparsed = text::deserialize(&text).unwrap();
    let text2 = text::serialize(&reparsed);
    assert_eq!(text, text2);
}

#[test]
fn parse_error_on_bad_version() {
    let input = "sair 0.2\nentry \"main\"\n";
    let err = text::deserialize(input).unwrap_err();
    assert!(err.to_string().contains("unsupported version"));
}

#[test]
fn parse_error_on_missing_entry() {
    let input = "sair 0.1\n";
    let err = text::deserialize(input).unwrap_err();
    assert!(err.to_string().contains("parse error"));
}

#[test]
fn roundtrip_debug_locs() {
    roundtrip(&module_with_debug_locs());
}

#[test]
fn roundtrip_calls() {
    roundtrip(&module_with_calls());
}

#[test]
fn debug_locs_preserved() {
    let module = module_with_debug_locs();
    let text = text::serialize(&module);
    assert!(text.contains("!loc \"src.c\" 10 5"), "serialized text should contain debug loc: {text}");
    let reparsed = text::deserialize(&text).unwrap();
    let block = &reparsed.functions[0].blocks[0];
    let loc = block.debug_loc(0).expect("debug loc should be preserved");
    assert_eq!(loc.file, "src.c");
    assert_eq!(loc.line, 10);
    assert_eq!(loc.column, Some(5));
}
