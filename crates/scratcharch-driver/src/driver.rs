use std::path::Path;

use scratcharch_core::value::Value as IsaValue;
use scratcharch_ir::lower::IsaLowerer;
use scratcharch_ir::r#module::{IrModule, StaticData, STATIC_DATA_BASE};
use scratcharch_opt::manager::PassManager;
use scratcharch_opt::{cfg_simplify::CfgSimplify, constant_fold::ConstantFold, dce::DeadCodeElimination};
use scratcharch_sair_interpreter::Interpreter;
use scratcharch_target::profile::TargetProfile;
use scratcharch_vm::vm::Vm;

pub use scratcharch_sair_interpreter::RuntimeValue as ExecutionValue;

use crate::config::{CompileConfig, ExecutionBackend, OptLevel};
use crate::error::DriverError;

/// A ScratchArch compilation driver.
///
/// The driver ties the LLVM frontend, SAIR optimizer, and one of two execution
/// backends into a single pipeline:
///
/// ```text
/// LLVM IR text → LLVM translator → SAIR module → optimization passes → interpreter
///                                                                    ↘ VM (lower to ISA)
/// ```
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
        match self.config.backend {
            ExecutionBackend::Interpreter => self.execute_interpreter(compiled),
            ExecutionBackend::Vm => self.execute_vm(compiled),
        }
    }

    fn execute_interpreter(
        &self,
        compiled: CompiledModule,
    ) -> Result<Option<ExecutionValue>, DriverError> {
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

    fn execute_vm(&self, compiled: CompiledModule) -> Result<Option<ExecutionValue>, DriverError> {
        let lowerer = IsaLowerer::with_profile(compiled.profile);
        let program = lowerer
            .lower(&compiled.module)
            .map_err(|e| DriverError::Vm(format!("lowering failed: {:?}", e)))?;
        let mut vm = Vm::new(self.config.memory_size, self.config.stack_limit);
        vm.load_program(&program)
            .map_err(|e| DriverError::Vm(format!("load failed: {}", e)))?;
        seed_vm_static(&mut vm, &compiled.module.static_data, self.config.stack_limit)
            .map_err(DriverError::Vm)?;
        vm.run().map_err(|e| DriverError::Vm(format!("execution failed: {}", e)))?;
        Ok(vm.stack.peek().ok().map(convert_vm_value))
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

fn convert_vm_value(value: &IsaValue) -> ExecutionValue {
    match *value {
        IsaValue::I1(v) => ExecutionValue::I1(v),
        IsaValue::I8(v) => ExecutionValue::I8(v),
        IsaValue::I16(v) => ExecutionValue::I16(v),
        IsaValue::I32(v) => ExecutionValue::I32(v),
        IsaValue::F64(v) => ExecutionValue::F64(v),
        IsaValue::Pointer(v) => ExecutionValue::Pointer(v),
    }
}

/// Seed the VM's static data segment (LLVM globals) before execution.
///
/// The VM's memory is byte-addressed and its `Load8`/`Store8` ops are
/// byte-granular, and every single-limb `Load`/`Store` in `IsaLowerer` is
/// width-accurate (`ty.size_in_bytes()`), so the byte image seeds exactly —
/// word-granular and sub-word/byte leaves alike. The segment must still sit
/// below the stack floor, or the stack could overwrite it later — reported as
/// an error, never allowed to collide.
fn seed_vm_static(
    vm: &mut Vm,
    data: &StaticData,
    stack_limit: u32,
) -> Result<(), String> {
    if data.image.is_empty() {
        return Ok(());
    }
    let size = data.image.len();
    let end = STATIC_DATA_BASE as usize + size;
    if end > stack_limit as usize {
        return Err(format!(
            "global static data ({} bytes at base {}) does not fit below the stack \
             floor ({}); raise the stack limit or shrink the data segment",
            size, STATIC_DATA_BASE, stack_limit
        ));
    }
    vm.memory
        .write(STATIC_DATA_BASE, &data.image)
        .map_err(|e| format!("seeding static data failed: {:?}", e))
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
