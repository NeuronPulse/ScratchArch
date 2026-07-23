use std::path::Path;

use scratcharch_ir::r#module::IrModule;
use scratcharch_opt::manager::PassManager;
use scratcharch_opt::{cfg_simplify::CfgSimplify, constant_fold::ConstantFold, dce::DeadCodeElimination};
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_target::profile::TargetProfile;

pub use scratcharch_sair_interpreter::RuntimeValue as ExecutionValue;

use crate::config::{CompileConfig, OptLevel};
use crate::error::DriverError;

/// A ScratchArch compilation driver.
///
/// The driver ties the LLVM frontend, SAIR optimizer, and SAIR interpreter into
/// a single pipeline. The VM backend can be added later without changing the
/// public API.
#[derive(Debug, Clone)]
pub struct CompileDriver {
    config: CompileConfig,
}

/// A compiled SAIR module ready for execution.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    pub module: IrModule,
    pub profile: TargetProfile,
}

impl CompileDriver {
    /// Create a driver with the given configuration.
    pub fn new(config: CompileConfig) -> Self {
        Self { config }
    }

    /// Compile LLVM IR text into a validated and optimized SAIR module.
    pub fn compile(&self, llvm_ir: &str) -> Result<CompiledModule, DriverError> {
        let mut module = scratcharch_llvm::translate_llvm(llvm_ir)
            .map_err(|e| DriverError::Llvm(e.to_string()))?;
        module
            .validate()
            .map_err(DriverError::Validation)?;
        let mut pm = build_pass_manager(self.config.opt_level);
        pm.run(&mut module);
        Ok(CompiledModule {
            module,
            profile: self.config.profile.clone(),
        })
    }

    /// Compile an LLVM IR file into a SAIR module.
    pub fn compile_file(&self, path: impl AsRef<Path>) -> Result<CompiledModule, DriverError> {
        let content = std::fs::read_to_string(path)?;
        self.compile(&content)
    }

    /// Execute a compiled module and return its exit value.
    pub fn execute(&self, compiled: CompiledModule) -> Result<Option<ExecutionValue>, DriverError> {
        let mut interp = Interpreter::new(
            compiled.module,
            self.config.memory_size,
            self.config.stack_limit,
        );
        interp.set_max_frames(self.config.max_frames);
        interp
            .run()
            .map_err(|e| DriverError::Execution(format!("{:?}", e)))
    }

    /// Compile LLVM IR text and immediately run it.
    pub fn compile_and_run(&self, llvm_ir: &str) -> Result<Option<ExecutionValue>, DriverError> {
        let compiled = self.compile(llvm_ir)?;
        self.execute(compiled)
    }

    /// Compile an LLVM IR file and immediately run it.
    pub fn compile_and_run_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<Option<ExecutionValue>, DriverError> {
        let compiled = self.compile_file(path)?;
        self.execute(compiled)
    }
}

impl Default for CompileDriver {
    fn default() -> Self {
        Self::new(CompileConfig::default())
    }
}

fn build_pass_manager(opt_level: OptLevel) -> PassManager {
    let mut pm = PassManager::new();
    match opt_level {
        OptLevel::None => {}
        OptLevel::Basic => {
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
        }
        OptLevel::Aggressive => {
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
            pm.add(CfgSimplify);
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
        }
    }
    pm
}
