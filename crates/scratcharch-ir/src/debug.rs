/// Optional debug location attached to a SAIR instruction.
///
/// Debug locations do not affect execution semantics; they are used for
/// diagnostics, profiling, and source-level debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugLoc {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

impl DebugLoc {
    pub fn new(file: impl Into<String>, line: u32, column: impl Into<Option<u32>>) -> Self {
        DebugLoc {
            file: file.into(),
            line,
            column: column.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_loc_with_column() {
        let loc = DebugLoc::new("src.c", 10, 5);
        assert_eq!(loc.file, "src.c");
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, Some(5));
    }

    #[test]
    fn debug_loc_without_column() {
        let loc = DebugLoc::new("src.c", 20, None);
        assert_eq!(loc.column, None);
    }
}
