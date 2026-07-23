use scratcharch_core::instruction::Instruction;
use scratcharch_core::program::{Program, Function};
use scratcharch_vm::vm::Vm;

fn run_program(program: &mut Program) -> Vm {
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(program).expect("failed to load program");
    vm.run().expect("failed to run program");
    vm
}

fn build_single_func_program(instructions: Vec<Instruction>) -> Program {
    let mut prog = Program::new("main");
    let mut func = Function::new("main");
    for instr in instructions {
        func.push(None::<&str>, instr);
    }
    prog.add_function(func);
    prog
}

#[test]
fn test_integer_arithmetic_42() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(20),
        Instruction::ConstI32(22),
        Instruction::I32Add,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    let result = vm.stack.peek().expect("stack should have a value");
    assert_eq!(result.as_i32(), Some(42));
}

#[test]
fn test_subtraction() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(50),
        Instruction::ConstI32(8),
        Instruction::I32Sub,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_multiplication() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(6),
        Instruction::ConstI32(7),
        Instruction::I32Mul,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_division() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(84),
        Instruction::ConstI32(2),
        Instruction::I32Div,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_remainder() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(47),
        Instruction::ConstI32(5),
        Instruction::I32Rem,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(2));
}

#[test]
fn test_wrapping_arithmetic() {
    // max i32 (4294967295) + 1 should wrap to 0
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(u32::MAX),
        Instruction::ConstI32(1),
        Instruction::I32Add,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0));
}

#[test]
fn test_wrapping_subtract_underflow() {
    // 0 - 1 should wrap to u32::MAX
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(0),
        Instruction::ConstI32(1),
        Instruction::I32Sub,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(u32::MAX));
}

#[test]
fn test_wrapping_multiply_overflow() {
    // large multiplication that wraps
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(0x1000000),
        Instruction::ConstI32(0x1000),
        Instruction::I32Mul,
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0));
}

#[test]
fn test_memory_alloc_store_load() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(4),   // size = 4 bytes
        Instruction::Alloc,          // allocate, addr on stack
        Instruction::Dup,            // duplicate address for later load
        Instruction::ConstI32(42),  // value to store
        Instruction::Store,          // store value at address (pops val, addr)
        Instruction::Load,           // load from address (the dup'd copy)
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_drop_and_dup() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(10),
        Instruction::ConstI32(20),
        Instruction::Drop,            // drop 20
        Instruction::Dup,             // duplicate 10
        Instruction::I32Add,          // 10 + 10 = 20
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(20));
}

#[test]
fn test_bitwise_and_or_xor() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(0xFF),
        Instruction::ConstI32(0x0F),
        Instruction::And,              // 0x0F
        Instruction::ConstI32(0xF0),
        Instruction::Or,               // 0xFF
        Instruction::ConstI32(0xFF),
        Instruction::Xor,              // 0x00
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0));
}

#[test]
fn test_shift_left_right() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(1),
        Instruction::ConstI32(4),
        Instruction::Shl,             // 16
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(16));

    let mut prog2 = build_single_func_program(vec![
        Instruction::ConstI32(0x100),
        Instruction::ConstI32(4),
        Instruction::Shr,             // 0x10
        Instruction::Return,
    ]);
    let vm2 = run_program(&mut prog2);
    assert_eq!(vm2.stack.peek().unwrap().as_i32(), Some(0x10));
}

#[test]
fn test_comparison_eq_lt_gt() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(5),
        Instruction::ConstI32(5),
        Instruction::Eq,              // true (1)
        Instruction::ConstI32(3),
        Instruction::ConstI32(5),
        Instruction::Lt,              // true (1)
        Instruction::ConstI32(7),
        Instruction::ConstI32(5),
        Instruction::Gt,              // true (1)
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    // Stack should have [1, 1, 1] from bottom to top
    assert_eq!(vm.stack.len(), 3);
}

#[test]
fn test_function_call_and_return() {
    let mut prog = Program::new("main");

    let mut add_fn = Function::new("add");
    add_fn.push(None::<&str>, Instruction::I32Add);
    add_fn.push(None::<&str>, Instruction::Return);
    prog.add_function(add_fn);

    let mut main_fn = Function::new("main");
    main_fn.push(None::<&str>, Instruction::ConstI32(20));
    main_fn.push(None::<&str>, Instruction::ConstI32(22));
    main_fn.push(None::<&str>, Instruction::Call("add".to_string()));
    main_fn.push(None::<&str>, Instruction::Return);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_conditional_branch() {
    let mut prog = Program::new("main");
    let mut main_fn = Function::new("main");

    // Push condition (true), then branch
    // If true: push 42, jump to end
    // If false: push 0, jump to end
    main_fn.push(None::<&str>, Instruction::ConstI32(10));
    main_fn.push(None::<&str>, Instruction::ConstI32(10));
    main_fn.push(None::<&str>, Instruction::Eq);    // true (1)
    main_fn.push(None::<&str>, Instruction::Branch("then".to_string(), "else".to_string()));
    main_fn.push(Some("then"), Instruction::ConstI32(42));
    main_fn.push(None::<&str>, Instruction::Jump("end".to_string()));
    main_fn.push(Some("else"), Instruction::ConstI32(0));
    main_fn.push(Some("end"), Instruction::Return);

    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_conditional_branch_false() {
    let mut prog = Program::new("main");
    let mut main_fn = Function::new("main");

    // Push condition (false), then branch
    main_fn.push(None::<&str>, Instruction::ConstI32(10));
    main_fn.push(None::<&str>, Instruction::ConstI32(20));
    main_fn.push(None::<&str>, Instruction::Eq);    // false (0)
    main_fn.push(None::<&str>, Instruction::Branch("then".to_string(), "else".to_string()));
    main_fn.push(Some("then"), Instruction::ConstI32(42));
    main_fn.push(None::<&str>, Instruction::Jump("end".to_string()));
    main_fn.push(Some("else"), Instruction::ConstI32(99));
    main_fn.push(Some("end"), Instruction::Return);

    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(99));
}

#[test]
fn test_division_by_zero_error() {
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(42),
        Instruction::ConstI32(0),
        Instruction::I32Div,
        Instruction::Return,
    ]);
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&mut prog).expect("failed to load program");
    let result = vm.run();
    assert!(result.is_err());
}

#[test]
fn test_unconditional_jump() {
    let mut prog = Program::new("main");
    let mut main_fn = Function::new("main");

    main_fn.push(None::<&str>, Instruction::ConstI32(10));
    main_fn.push(None::<&str>, Instruction::Jump("skip".to_string()));
    main_fn.push(None::<&str>, Instruction::ConstI32(99)); // skipped
    main_fn.push(Some("skip"), Instruction::Return);

    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(10));
}

#[test]
fn test_nested_function_calls() {
    let mut prog = Program::new("main");

    let mut add_one = Function::new("add_one");
    add_one.push(None::<&str>, Instruction::ConstI32(1));
    add_one.push(None::<&str>, Instruction::I32Add);
    add_one.push(None::<&str>, Instruction::Return);
    prog.add_function(add_one);

    let mut main_fn = Function::new("main");
    main_fn.push(None::<&str>, Instruction::ConstI32(40));
    main_fn.push(None::<&str>, Instruction::Call("add_one".to_string()));
    main_fn.push(None::<&str>, Instruction::Return);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(41));
}

#[test]
fn test_recursive_call() {
    let mut prog = Program::new("main");

    // factorial: if n <= 1, return 1; else return n * fact(n-1)
    let mut fact = Function::new("fact");
    // Stack at start: [n]
    // Duplicate n for comparison
    fact.push(None::<&str>, Instruction::Dup);      // [n, n]
    fact.push(None::<&str>, Instruction::ConstI32(2));
    fact.push(None::<&str>, Instruction::Lt);        // [n, n < 2]  (i.e., n <= 1)
    fact.push(None::<&str>, Instruction::Branch("base".to_string(), "recur".to_string()));

    fact.push(Some("base"), Instruction::Drop);      // drop n
    fact.push(None::<&str>, Instruction::ConstI32(1)); // return 1
    fact.push(None::<&str>, Instruction::Return);

    // recur: n * fact(n-1)
    fact.push(Some("recur"), Instruction::Dup);      // [n, n]
    fact.push(None::<&str>, Instruction::ConstI32(1));
    fact.push(None::<&str>, Instruction::I32Sub);    // [n, n-1]
    fact.push(None::<&str>, Instruction::Call("fact".to_string())); // [n, fact(n-1)]
    fact.push(None::<&str>, Instruction::I32Mul);    // [n * fact(n-1)]
    fact.push(None::<&str>, Instruction::Return);

    prog.add_function(fact);

    let mut main_fn = Function::new("main");
    main_fn.push(None::<&str>, Instruction::ConstI32(5));
    main_fn.push(None::<&str>, Instruction::Call("fact".to_string()));
    main_fn.push(None::<&str>, Instruction::Return);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(120));
}
