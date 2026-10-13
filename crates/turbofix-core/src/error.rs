//! Structured errors. `CoreError` is the internal type; `ApiError` is the
//! `{code, message}` shape the frontend receives.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::InvalidConfig(_) => "invalid_config",
            CoreError::NotFound(_) => "not_found",
            CoreError::Cancelled => "cancelled",
            CoreError::Storage(_) => "storage",
            CoreError::PermissionDenied(_) => "permission_denied",
            CoreError::Internal(_) => "internal",
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Storage(e.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Internal(format!("serialization: {e}"))
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => CoreError::PermissionDenied(e.to_string()),
            std::io::ErrorKind::NotFound => CoreError::NotFound(e.to_string()),
            _ => CoreError::Internal(e.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        ApiError {
            code: e.code().to_string(),
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_messages_are_stable() {
        let a = ApiError::from(CoreError::InvalidConfig("probe_count".into()));
        assert_eq!(a.code, "invalid_config");
        assert_eq!(a.message, "invalid configuration: probe_count");
        assert_eq!(ApiError::from(CoreError::Cancelled).code, "cancelled");
        assert_eq!(
            ApiError::from(CoreError::NotFound("scan 1".into())).code,
            "not_found"
        );
    }

    #[test]
    fn io_errors_map_to_domain_errors() {
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(CoreError::from(denied).code(), "permission_denied");
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(CoreError::from(missing).code(), "not_found");
        assert_eq!(
            CoreError::from(std::io::Error::other("x")).code(),
            "internal"
        );
    }

    #[test]
    fn sqlite_errors_map_to_storage() {
        let e = rusqlite::Connection::open_in_memory()
            .unwrap()
            .execute("NOT SQL", [])
            .unwrap_err();
        assert_eq!(CoreError::from(e).code(), "storage");
    }

    #[test]
    fn api_error_serialises_as_code_and_message() {
        let v = serde_json::to_value(ApiError::from(CoreError::Cancelled)).unwrap();
        assert_eq!(v["code"], "cancelled");
    }
}
