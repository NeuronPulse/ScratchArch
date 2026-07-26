use std::fmt;

use serde::Serialize;

/// Severity level of a compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticLevel::Error => write!(f, "error"),
            DiagnosticLevel::Warning => write!(f, "warning"),
            DiagnosticLevel::Note => write!(f, "note"),
        }
    }
}

/// A single diagnostic message.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub stage: Option<String>,
    pub source_file: Option<String>,
    pub source_line: Option<u32>,
    pub source_column: Option<u32>,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            stage: None,
            source_file: None,
            source_line: None,
            source_column: None,
            suggestion: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            stage: None,
            source_file: None,
            source_line: None,
            source_column: None,
            suggestion: None,
        }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Note,
            message: message.into(),
            stage: None,
            source_file: None,
            source_line: None,
            source_column: None,
            suggestion: None,
        }
    }

    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    pub fn with_source(mut self, file: impl Into<String>, line: u32, column: impl Into<Option<u32>>) -> Self {
        self.source_file = Some(file.into());
        self.source_line = Some(line);
        self.source_column = column.into();
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.level, self.message)?;
        if let Some(stage) = &self.stage {
            write!(f, " [{}]", stage)?;
        }
        if let (Some(file), Some(line)) = (&self.source_file, self.source_line) {
            write!(f, "\n --> {}:{}", file, line)?;
            if let Some(col) = self.source_column {
                write!(f, ":{}", col)?;
            }
        }
        if let Some(suggestion) = &self.suggestion {
            write!(f, "\n help: {}", suggestion)?;
        }
        Ok(())
    }
}

/// Collects diagnostics and can emit them as text or JSON.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticSink {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    pub fn new() -> Self {
        Self { diagnostics: Vec::new() }
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.push(Diagnostic::error(message));
    }

    pub fn push_warning(&mut self, message: impl Into<String>) {
        self.push(Diagnostic::warning(message));
    }

    pub fn push_note(&mut self, message: impl Into<String>) {
        self.push(Diagnostic::note(message));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == DiagnosticLevel::Error)
    }

    pub fn to_text(&self) -> String {
        self.diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.diagnostics)
    }

    pub fn into_result<T>(self, value: T) -> Result<T, Vec<Diagnostic>> {
        if self.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(value)
        }
    }
}
