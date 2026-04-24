use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("Unknown format")]
    UnknownFormat,
    #[error("Invalid trajectory: {0}")]
    InvalidTrajectory(String),
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(e: serde_json::Error) -> Self {
        ParseError::Json(e.to_string())
    }
}
