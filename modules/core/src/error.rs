use std::fmt;

use thiserror::Error;

/// Primary error type for OpenCAD operations.
#[derive(Debug, Error)]
pub enum OpenCadError {
    #[error("invalid id: {0}")]
    InvalidId(String),

    #[error("invalid unit: {0}")]
    InvalidUnit(String),

    #[error("invalid expression: {0}")]
    InvalidExpression(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("transaction error: {0}")]
    Transaction(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("{0}")]
    Other(String),
}

/// Convenience result alias used across OpenCAD crates.
pub type Result<T> = std::result::Result<T, OpenCadError>;

impl OpenCadError {
    pub fn validation(message: impl fmt::Display) -> Self {
        Self::Validation(message.to_string())
    }

    pub fn transaction(message: impl fmt::Display) -> Self {
        Self::Transaction(message.to_string())
    }

    pub fn not_found(message: impl fmt::Display) -> Self {
        Self::NotFound(message.to_string())
    }

    /// Prefix the message with `context` while keeping the error kind, so
    /// callers that match on the variant still see the original category.
    pub fn with_context(self, context: impl fmt::Display) -> Self {
        let prefix = |message: String| format!("{context}: {message}");
        match self {
            Self::InvalidId(message) => Self::InvalidId(prefix(message)),
            Self::InvalidUnit(message) => Self::InvalidUnit(prefix(message)),
            Self::InvalidExpression(message) => Self::InvalidExpression(prefix(message)),
            Self::Validation(message) => Self::Validation(prefix(message)),
            Self::Transaction(message) => Self::Transaction(prefix(message)),
            Self::NotFound(message) => Self::NotFound(prefix(message)),
            Self::Other(message) => Self::Other(prefix(message)),
            other @ (Self::Serialization(_) | Self::ChecksumMismatch { .. }) => {
                Self::Other(prefix(other.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_context_keeps_the_error_kind() {
        let error = OpenCadError::validation("depth must be positive")
            .with_context("feature 'feature:bore'");
        assert!(matches!(error, OpenCadError::Validation(_)));
        assert_eq!(
            error.to_string(),
            "validation failed: feature 'feature:bore': depth must be positive"
        );
    }

    #[test]
    fn with_context_wraps_structured_errors_as_other() {
        let error = OpenCadError::ChecksumMismatch {
            expected: "a".into(),
            actual: "b".into(),
        }
        .with_context("graph/features.json");
        assert_eq!(
            error.to_string(),
            "graph/features.json: checksum mismatch: expected a, got b"
        );
    }
}
