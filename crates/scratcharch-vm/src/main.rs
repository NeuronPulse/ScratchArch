use std::process;
use scratcharch_core::instruction::Instruction;
use scratcharch_core::program::{Program, Function};
use scratcharch_vm::vm::Vm;

fn main() {
    let mut program = Program::new("main");

    let mut main_fn = Function::new("main");

    // Push 20 and 22, add them, drop result
    main_fn.push(None::<&str>, Instruction::ConstI32(20));
    main_fn.push(None::<&str>, Instruction::ConstI32(22));
    main_fn.push(None::<&str>, Instruction::I32Add);
    #[allow(clippy::approx_constant)]
    main_fn.push(None::<&str>, Instruction::ConstF64(3.14));
    main_fn.push(None::<&str>, Instruction::Drop);
    main_fn.push(None::<&str>, Instruction::Return);
    program.add_function(main_fn);

    let mut vm = Vm::new(65536, 4096);
    if let Err(e) = vm.load_program(&program) {
        eprintln!("Error loading program: {e}");
        process::exit(1);
    }

    if let Err(e) = vm.run() {
        eprintln!("Runtime error: {e}");
        process::exit(1);
    }

    println!("Final stack:");
    for v in vm.stack.iter() {
        println!("  {v}");
    }
}
