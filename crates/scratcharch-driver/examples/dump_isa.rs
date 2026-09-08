use scratcharch_driver::{CompileConfig, CompileDriver};
use scratcharch_ir::lower::IsaLowerer;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(path).unwrap();
    let config = CompileConfig { ..CompileConfig::default() };
    let driver = CompileDriver::new(config);
    let compiled = driver.compile(&src).unwrap();
    let lowerer = IsaLowerer::with_profile(compiled.profile.clone());
    let program = lowerer.lower(&compiled.module).unwrap();
    for func in &program.functions {
        println!("== function {} ==", func.name);
        for (i, (label, instr)) in func.instructions.iter().enumerate() {
            println!("{i:3} {label:>20?}  {instr}");
        }
    }
}
