use scratcharch_ir::r#module::IrModule;

pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(&mut self, module: &mut IrModule);
}
