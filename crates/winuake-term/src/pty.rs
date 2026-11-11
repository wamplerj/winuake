//! Pseudo-terminal abstraction. The Windows ConPTY implementation lives in `conpty`; tests use a fake.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// Terminal dimensions in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}

impl TerminalSize {
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
}

/// What to launch inside a new pseudo-terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtySpawnRequest {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub environment: Vec<EnvironmentVariable>,
    pub size: TerminalSize,
}

/// How the child process ended. `code` is `None` when unknown (for example, killed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("failed to start `{program}`: {reason}")]
    SpawnFailed { program: String, reason: String },
    #[error("pseudo-terminal operation `{operation}` failed: {reason}")]
    OperationFailed { operation: &'static str, reason: String },
    #[error("pseudo-terminals are not supported on this platform")]
    Unsupported,
}

/// Out-of-band control of a running pseudo-terminal. Shared between the session and its reader thread.
pub trait PtyControl: Send + Sync {
    fn resize(&self, size: TerminalSize) -> Result<(), PtyError>;
    /// Blocks until the child process has exited.
    fn wait_for_exit(&self) -> ExitStatus;
    fn kill(&self) -> Result<(), PtyError>;
}

/// The three ends of a live pseudo-terminal: child output, child input, and control.
pub struct PtyConnection {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub control: Arc<dyn PtyControl>,
}

pub trait PtyBackend: Send + Sync {
    fn spawn(&self, request: &PtySpawnRequest) -> Result<PtyConnection, PtyError>;
}
