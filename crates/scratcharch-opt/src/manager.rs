use crate::pass::OptimizationPass;
use scratcharch_ir::r#module::IrModule;

pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl PassManager {
    pub fn new() -> Self {
        PassManager { passes: Vec::new() }
    }

    pub fn add<P: OptimizationPass + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }

    pub fn run(&mut self, module: &mut IrModule) {
        for pass in &mut self.passes {
            pass.run(module);
        }
    }

    pub fn print_pipeline(&self) {
        eprintln!("Pass pipeline:");
        for (i, pass) in self.passes.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, pass.name());
        }
    }

    pub fn passes(&self) -> &[Box<dyn OptimizationPass>] {
        &self.passes
    }
}

impl Default for PassManager {
    fn default() -> Self {
        Self::new()
    }
}
