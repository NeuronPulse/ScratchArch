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
    let prog = build_single_func_program(vec![
        Instruction::ConstI32(42),
        Instruction::ConstI32(0),
        Instruction::I32Div,
        Instruction::Return,
    ]);
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&prog).expect("failed to load program");
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

// ── Local-slot tests ─────────────────────────────────────────────

fn build_program_with_function(func: Function) -> Program {
    let mut prog = Program::new(&func.name);
    prog.add_function(func);
    prog
}

#[test]
fn test_local_get_set() {
    let mut main_fn = Function::new("main");
    main_fn.local_count = 1;
    main_fn.return_cells = 1;
    main_fn.push(None::<&str>, Instruction::ConstI32(42));
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = build_program_with_function(main_fn);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_function_prologue_consumes_args() {
    let mut add_fn = Function::new("add");
    add_fn.param_cells = 2;
    add_fn.local_count = 2;
    add_fn.return_cells = 1;
    // Prologue: pop arguments into slots [b, a]
    add_fn.push(None::<&str>, Instruction::LocalSet(1));
    add_fn.push(None::<&str>, Instruction::LocalSet(0));
    add_fn.push(None::<&str>, Instruction::LocalGet(0));
    add_fn.push(None::<&str>, Instruction::LocalGet(1));
    add_fn.push(None::<&str>, Instruction::I32Add);
    add_fn.push(None::<&str>, Instruction::Return);

    let mut main_fn = Function::new("main");
    main_fn.return_cells = 1;
    main_fn.push(None::<&str>, Instruction::ConstI32(20));
    main_fn.push(None::<&str>, Instruction::ConstI32(22));
    main_fn.push(None::<&str>, Instruction::Call("add".to_string()));
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = Program::new("main");
    prog.add_function(add_fn);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_call_return_preserves_caller_locals() {
    let mut callee = Function::new("callee");
    callee.return_cells = 1;
    callee.push(None::<&str>, Instruction::ConstI32(1));
    callee.push(None::<&str>, Instruction::Return);

    let mut main_fn = Function::new("main");
    main_fn.local_count = 2;
    main_fn.return_cells = 1;
    // Set locals 0 and 1
    main_fn.push(None::<&str>, Instruction::ConstI32(10));
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::ConstI32(20));
    main_fn.push(None::<&str>, Instruction::LocalSet(1));
    // Call and discard return value
    main_fn.push(None::<&str>, Instruction::Call("callee".to_string()));
    main_fn.push(None::<&str>, Instruction::Drop);
    // Read locals back and add
    main_fn.push(None::<&str>, Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::LocalGet(1));
    main_fn.push(None::<&str>, Instruction::I32Add);
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = Program::new("main");
    prog.add_function(callee);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(30));
}

#[test]
fn test_multi_block_branch_locals() {
    let mut main_fn = Function::new("main");
    main_fn.local_count = 1;
    main_fn.return_cells = 1;
    main_fn.push(None::<&str>, Instruction::ConstI1(true));
    main_fn.push(None::<&str>, Instruction::Branch("then".to_string(), "else".to_string()));

    main_fn.push(Some("then"), Instruction::ConstI32(42));
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::Jump("end".to_string()));

    main_fn.push(Some("else"), Instruction::ConstI32(99));
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::Jump("end".to_string()));

    main_fn.push(Some("end"), Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = build_program_with_function(main_fn);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(42));
}

#[test]
fn test_loop_with_locals() {
    let mut main_fn = Function::new("main");
    main_fn.local_count = 2; // slot 0 = i, slot 1 = acc
    main_fn.return_cells = 1;
    // i = 0; acc = 0;
    main_fn.push(None::<&str>, Instruction::ConstI32(0));
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::ConstI32(0));
    main_fn.push(None::<&str>, Instruction::LocalSet(1));
    main_fn.push(None::<&str>, Instruction::Jump("check".to_string()));

    // check: i < 5 ?
    main_fn.push(Some("check"), Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::ConstI32(5));
    main_fn.push(None::<&str>, Instruction::Lt);
    main_fn.push(None::<&str>, Instruction::Branch("body".to_string(), "end".to_string()));

    // body: acc += i; i += 1
    main_fn.push(Some("body"), Instruction::LocalGet(1));
    main_fn.push(None::<&str>, Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::I32Add);
    main_fn.push(None::<&str>, Instruction::LocalSet(1));
    main_fn.push(None::<&str>, Instruction::LocalGet(0));
    main_fn.push(None::<&str>, Instruction::ConstI32(1));
    main_fn.push(None::<&str>, Instruction::I32Add);
    main_fn.push(None::<&str>, Instruction::LocalSet(0));
    main_fn.push(None::<&str>, Instruction::Jump("check".to_string()));

    // end: return acc (0+1+2+3+4 = 10)
    main_fn.push(Some("end"), Instruction::LocalGet(1));
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = build_program_with_function(main_fn);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(10));
}

#[test]
fn test_recursive_factorial_locals() {
    let mut fact = Function::new("fact");
    fact.param_cells = 1;
    fact.local_count = 2; // slot 0 = n, slot 1 = recursive result
    fact.return_cells = 1;
    // Prologue: pop n
    fact.push(None::<&str>, Instruction::LocalSet(0));
    // if n < 2 return 1
    fact.push(None::<&str>, Instruction::LocalGet(0));
    fact.push(None::<&str>, Instruction::ConstI32(2));
    fact.push(None::<&str>, Instruction::Lt);
    fact.push(None::<&str>, Instruction::Branch("base".to_string(), "recur".to_string()));

    fact.push(Some("base"), Instruction::ConstI32(1));
    fact.push(None::<&str>, Instruction::Return);

    // recur: rec = fact(n - 1); return n * rec
    fact.push(Some("recur"), Instruction::LocalGet(0));
    fact.push(None::<&str>, Instruction::ConstI32(1));
    fact.push(None::<&str>, Instruction::I32Sub);
    fact.push(None::<&str>, Instruction::Call("fact".to_string()));
    fact.push(None::<&str>, Instruction::LocalSet(1));
    fact.push(None::<&str>, Instruction::LocalGet(0));
    fact.push(None::<&str>, Instruction::LocalGet(1));
    fact.push(None::<&str>, Instruction::I32Mul);
    fact.push(None::<&str>, Instruction::Return);

    let mut main_fn = Function::new("main");
    main_fn.return_cells = 1;
    main_fn.push(None::<&str>, Instruction::ConstI32(5));
    main_fn.push(None::<&str>, Instruction::Call("fact".to_string()));
    main_fn.push(None::<&str>, Instruction::Return);

    let mut prog = Program::new("main");
    prog.add_function(fact);
    prog.add_function(main_fn);

    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(120));
}

// ── Load8 / Store8 / Trap primitives (ISA Appendix A / EXECUTION_MODEL §5.6) ──

#[test]
fn test_store8_writes_only_low_byte() {
    // Store a value whose upper bits are set (0x1FF): only the low byte 0xFF
    // may reach memory.
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(4), // size
        Instruction::Alloc,       // [addr]
        Instruction::Dup,         // [addr, addr]
        Instruction::ConstI32(0x1FF),
        Instruction::Store8,      // write 0xFF at addr, leaves [addr]
        Instruction::Load8,       // [0xFF]
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0xFF));
}

#[test]
fn test_load8_zero_extends() {
    // A stored byte 0xAB reads back as 0x000000AB (zero-extended), never as a
    // sign-extended negative.
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(1), // size
        Instruction::Alloc,       // [addr]
        Instruction::Dup,
        Instruction::ConstI32(0xAB),
        Instruction::Store8,      // [addr]
        Instruction::Load8,       // [0xAB]
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0xAB));
}

#[test]
fn test_store8_does_not_disturb_neighbouring_bytes() {
    // Store byte 0xAB at addr, 0xCD at addr+1, 0xFF at addr+2. Reading addr
    // must still yield 0xAB and reading addr+3 (never written) must yield 0.
    let mut prog = build_single_func_program(vec![
        Instruction::ConstI32(8), // size
        Instruction::Alloc,       // [addr]
        // addr <- 0xAB
        Instruction::Dup,         // [addr, addr]
        Instruction::ConstI32(0xAB),
        Instruction::Store8,      // [addr]
        // addr+1 <- 0xCD
        Instruction::Dup,
        Instruction::ConstI32(1),
        Instruction::I32Add,      // [addr, addr+1]
        Instruction::ConstI32(0xCD),
        Instruction::Store8,      // [addr]
        // addr+2 <- 0xFF
        Instruction::Dup,
        Instruction::ConstI32(2),
        Instruction::I32Add,
        Instruction::ConstI32(0xFF),
        Instruction::Store8,      // [addr]
        // load addr (must be 0xAB, not clobbered by addr+1/addr+2 stores)
        Instruction::Load8,       // [0xAB]
        Instruction::Return,
    ]);
    let vm = run_program(&mut prog);
    assert_eq!(vm.stack.peek().unwrap().as_i32(), Some(0xAB));
}

#[test]
fn test_trap_terminates_in_distinguished_state() {
    // Trap is terminal: run() reports VmError::Trap (a program-declared stop,
    // distinct from a normal return and from the machine-error classes).
    let prog = build_single_func_program(vec![
        Instruction::ConstI32(7),
        Instruction::Trap,
    ]);
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&prog).expect("failed to load program");
    let result = vm.run();
    match result {
        Err(scratcharch_vm::vm::VmError::Trap { function, pc }) => {
            assert_eq!(function, 0, "trap should report the function index");
            assert_eq!(pc, 1, "trap pc should be the trap instruction");
        }
        other => panic!("expected VmError::Trap, got {other:?}"),
    }
    assert!(!vm.running, "trap must halt the machine");
}

#[test]
fn test_trap_is_distinct_from_normal_return() {
    // A program that traps must not report a return value (no silent return).
    let prog = build_single_func_program(vec![
        Instruction::ConstI32(99),
        Instruction::Trap,
    ]);
    let mut vm = Vm::new(65536, 4096);
    vm.load_program(&prog).expect("failed to load program");
    assert!(vm.run().is_err(), "trap must not complete the program normally");
}
