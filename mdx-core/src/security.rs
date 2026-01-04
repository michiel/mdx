//! Security event tracking and warnings

use serde::{Deserialize, Serialize};

/// Security event severity level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityEventLevel {
    /// Informational event
    Info,
    /// Warning that should be reviewed
    Warning,
    /// Error or security violation
    Error,
}

/// A security-related event that occurred during operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    /// Severity level of the event
    pub level: SecurityEventLevel,
    /// Human-readable message
    pub message: String,
    /// Source of the event (e.g., "config", "document", "editor")
    pub source: String,
}

impl SecurityEvent {
    /// Create a new security event
    pub fn new(
        level: SecurityEventLevel,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            level,
            message: message.into(),
            source: source.into(),
        }
    }

    /// Create a warning event
    pub fn warning(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SecurityEventLevel::Warning, message, source)
    }

    /// Create an info event
    pub fn info(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SecurityEventLevel::Info, message, source)
    }

    /// Create an error event
    pub fn error(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SecurityEventLevel::Error, message, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_warning() {
        let event = SecurityEvent::warning("Test warning", "test");
        assert_eq!(event.level, SecurityEventLevel::Warning);
        assert_eq!(event.message, "Test warning");
        assert_eq!(event.source, "test");
    }

    #[test]
    fn test_create_info() {
        let event = SecurityEvent::info("Test info", "test");
        assert_eq!(event.level, SecurityEventLevel::Info);
    }

    #[test]
    fn test_create_error() {
        let event = SecurityEvent::error("Test error", "test");
        assert_eq!(event.level, SecurityEventLevel::Error);
    }
}
