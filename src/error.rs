use std::io;
use thiserror::Error;

/// Core error type for ContextSmith.
#[derive(Error, Debug)]
pub enum ContextSmithError {
    #[error("config error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("I/O error: {context}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },

    #[error("invalid path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },

    #[error("validation error on '{field}': {message}")]
    Validation { field: String, message: String },

    #[error("git error: {message}")]
    Git { message: String },

    #[error("AST parsing error in '{file}': {message}")]
    AstParsing { file: String, message: String },

    #[error("tokenization error: {message}")]
    Tokenization { message: String },

    #[error("budget exceeded: requested {requested}, available {available}")]
    BudgetExceeded { requested: usize, available: usize },

    #[error("command '{command}' is not yet implemented")]
    NotImplemented { command: String },
}

impl ContextSmithError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    pub fn config_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    pub fn invalid_path(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidPath {
            path: path.into(),
            reason: reason.into(),
        }
    }

    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn not_implemented(command: impl Into<String>) -> Self {
        Self::NotImplemented {
            command: command.into(),
        }
    }

    /// Returns true if this error is caused by user input (vs internal/system).
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidPath { .. } | Self::Validation { .. } | Self::BudgetExceeded { .. }
        )
    }

    /// Returns true if retrying the operation might succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io { .. })
    }
}

pub type Result<T> = std::result::Result<T, ContextSmithError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_display() {
        let err = ContextSmithError::config("bad value");
        assert_eq!(err.to_string(), "config error: bad value");
    }

    #[test]
    fn not_implemented_display() {
        let err = ContextSmithError::not_implemented("diff");
        assert_eq!(err.to_string(), "command 'diff' is not yet implemented");
    }

    #[test]
    fn user_error_classification() {
        assert!(ContextSmithError::invalid_path("/bad", "nope").is_user_error());
        assert!(ContextSmithError::validation("field", "bad").is_user_error());
        assert!(!ContextSmithError::config("oops").is_user_error());
        assert!(!ContextSmithError::not_implemented("x").is_user_error());
    }

    #[test]
    fn retryable_classification() {
        let io_err = ContextSmithError::io("read", io::Error::new(io::ErrorKind::Other, "timeout"));
        assert!(io_err.is_retryable());
        assert!(!ContextSmithError::config("nope").is_retryable());
    }
}
