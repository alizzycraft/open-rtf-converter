use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub byte_offset: Option<usize>,
}

impl Diagnostic {
    pub fn warning(message: impl Into<String>, byte_offset: Option<usize>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            byte_offset,
        }
    }

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
