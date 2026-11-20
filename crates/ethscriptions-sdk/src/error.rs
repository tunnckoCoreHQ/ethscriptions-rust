use serde::Serialize;

/// API error type used across the SDK and server.
#[derive(Debug, thiserror::Error, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ApiError {
    #[error("Not found: {message}")]
    NotFound { message: String, http_status: u16 },

    #[error("Bad request: {message}")]
    BadRequest { message: String, http_status: u16 },

    #[error("Upstream error: {message}")]
    UpstreamError { message: String, http_status: u16 },

    #[error("Internal error: {message}")]
    Internal { message: String, http_status: u16 },
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
            http_status: 404,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
            http_status: 400,
        }
    }

    pub fn upstream_error(message: impl Into<String>) -> Self {
        Self::UpstreamError {
            message: message.into(),
            http_status: 502,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            http_status: 500,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound { http_status, .. } => *http_status,
            Self::BadRequest { http_status, .. } => *http_status,
            Self::UpstreamError { http_status, .. } => *http_status,
            Self::Internal { http_status, .. } => *http_status,
        }
    }
}
