//! Custom error types for Huginn CLI

use thiserror::Error;

/// Errors that can occur during configuration operations
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Cannot determine config directory paths")]
    CannotDeterminePaths,

    #[error("Failed to read config file: {0}")]
    ReadError(#[source] std::io::Error),

    #[error("Failed to parse config JSON: {0}")]
    ParseError(#[source] serde_json::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[source] serde_json::Error),

    #[error("Failed to create config directory: {0}")]
    CreateDirError(#[source] std::io::Error),

    #[error("Failed to write config file: {0}")]
    WriteError(#[source] std::io::Error),
}

/// Errors that can occur during terminal operations
#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Failed to enable raw mode: {0}")]
    EnableRawMode(#[source] std::io::Error),

    #[error("Failed to disable raw mode: {0}")]
    DisableRawMode(#[source] std::io::Error),

    #[error("Failed to enter alternate screen: {0}")]
    EnterAlternateScreen(#[source] std::io::Error),

    #[error("Failed to leave alternate screen: {0}")]
    LeaveAlternateScreen(#[source] std::io::Error),

    #[error("Failed to create terminal: {0}")]
    CreateTerminal(#[source] std::io::Error),

    #[error("Failed to draw to terminal: {0}")]
    DrawError(#[source] std::io::Error),

    #[error("PTY error: {0}")]
    PtyError(String),
}

/// Top-level error type for Huginn
#[derive(Debug, Error)]
pub enum HuginnError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Terminal error: {0}")]
    Terminal(#[from] TerminalError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for Huginn operations
pub type Result<T> = std::result::Result<T, HuginnError>;
