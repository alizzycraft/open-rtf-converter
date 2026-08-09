use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Severity assigned to a non-fatal conversion diagnostic.
pub enum Severity {
    /// Conversion continued with a visible approximation or policy action.
    Warning,
    /// A serious condition recorded for a caller-controlled reporting workflow.
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Bounded, payload-safe information about an approximation or policy action.
pub struct Diagnostic {
    /// Diagnostic severity.
    pub severity: Severity,
    /// Human-readable message that does not include raw untrusted payload data.
    pub message: String,
    /// Optional byte offset in the original RTF input.
    pub byte_offset: Option<usize>,
}

impl Diagnostic {
    /// Creates a warning at an optional input byte offset.
    pub fn warning(message: impl Into<String>, byte_offset: Option<usize>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            byte_offset,
        }
    }

    /// Creates an error diagnostic at an optional input byte offset.
    pub fn error(message: impl Into<String>, byte_offset: Option<usize>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            byte_offset,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };

        match self.byte_offset {
            Some(offset) => write!(f, "{level} at byte {offset}: {}", self.message),
            None => write!(f, "{level}: {}", self.message),
        }
    }
}
