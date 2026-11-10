use serde::{Serialize, Serializer};

/// Errors returned to the frontend. Serialized as a plain message string so the UI can show it directly.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse command output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),
    #[error("Command failed (exit code {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
