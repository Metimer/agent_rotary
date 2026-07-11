use thiserror::Error;

pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("Provider '{0}' not found in registry")]
    ProviderNotFound(String),

    #[error("Node '{0}' not found in workflow")]
    NodeNotFound(String),

    #[error("Workflow dead-end at node '{0}' (no outgoing edge satisfied)")]
    DeadEnd(String),

    #[error("Max iterations ({0}) reached — possible feedback loop runaway")]
    MaxIterations(usize),

    #[error("Missing required context key: {0}")]
    MissingContext(String),

    #[error("Template render error: {0}")]
    Template(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Provider '{provider}' returned an error (status {status}): {body}")]
    ProviderStatus {
        provider: String,
        status: u16,
        body: String,
    },

    #[error("Failed to parse provider response: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<reqwest::Error> for OrchestratorError {
    fn from(e: reqwest::Error) -> Self {
        OrchestratorError::Http(e.to_string())
    }
}

impl From<OrchestratorError> for pyo3::PyErr {
    fn from(e: OrchestratorError) -> Self {
        use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
        match e {
            OrchestratorError::ProviderNotFound(_)
            | OrchestratorError::NodeNotFound(_)
            | OrchestratorError::MissingContext(_) => PyKeyError::new_err(e.to_string()),
            OrchestratorError::Config(_) | OrchestratorError::Template(_) => {
                PyValueError::new_err(e.to_string())
            }
            other => PyRuntimeError::new_err(other.to_string()),
        }
    }
}
