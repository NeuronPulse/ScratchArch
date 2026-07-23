use scratcharch_target::profile::TargetProfile;

/// Optimization level for the compilation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No optimization passes.
    None,
    /// Basic constant folding and dead-code elimination.
    Basic,
    /// Aggressive optimization: constant folding, DCE, CFG simplification, and
    /// a second fold/DCE pass.
    Aggressive,
}

/// Configuration for the ScratchArch compilation driver.
#[derive(Debug, Clone)]
pub struct CompileConfig {
    /// Optimization level applied to SAIR before execution.
    pub opt_level: OptLevel,
    /// Total interpreter memory in bytes.
    pub memory_size: u32,
    /// Stack guard: the interpreter will not allocate below this address.
    pub stack_limit: u32,
    /// Maximum number of call frames.
    pub max_frames: usize,
    /// Target profile for the compilation.
    pub profile: TargetProfile,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            opt_level: OptLevel::Basic,
            memory_size: 65536,
            stack_limit: 4096,
            max_frames: 1024,
            profile: TargetProfile::sa48(),
        }
    }
}
