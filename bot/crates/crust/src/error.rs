use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrustError {
    #[error("Kube error: {0}")]
    Kube(#[from] kube::Error),
    #[error("NATS error: {0}")]
    Nats(#[from] async_nats::Error),
    #[error("Discord error: {0}")]
    Discord(#[from] twilight_http::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("General error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for CrustError {
    fn from(err: anyhow::Error) -> Self {
        CrustError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CrustError>;
