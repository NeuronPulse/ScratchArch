pub mod errors;
pub mod parser;
pub mod translator;

use scratcharch_ir::r#module::IrModule;
use crate::errors::LlvmError;

pub fn translate_llvm(input: &str) -> Result<IrModule, LlvmError> {
    let program = parser::parse_llvm(input)?;
    translator::translate(&program)
}
