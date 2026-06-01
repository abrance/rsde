use super::dto::{ApiErrorCode, ApiResponse};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, Clone)]
pub enum ObjectStorageError {
    ConfigError(String),
    InvalidInput(String),
    StorageError(String),
    UploadError(String),
    DownloadError(String),
    DeleteError(String),
    NotFound(String),
    ObjectConflict(String),
}

impl std::fmt::Display for ObjectStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectStorageError::ConfigError(msg) => write!(f, "Config error: {msg}"),
            ObjectStorageError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ObjectStorageError::StorageError(msg) => write!(f, "Storage error: {msg}"),
            ObjectStorageError::UploadError(msg) => write!(f, "Upload error: {msg}"),
            ObjectStorageError::DownloadError(msg) => write!(f, "Download error: {msg}"),
            ObjectStorageError::DeleteError(msg) => write!(f, "Delete error: {msg}"),
            ObjectStorageError::NotFound(msg) => write!(f, "Not found: {msg}"),
            ObjectStorageError::ObjectConflict(msg) => write!(f, "Object conflict: {msg}"),
        }
    }
}

impl std::error::Error for ObjectStorageError {}

impl IntoResponse for ObjectStorageError {
    fn into_response(self) -> Response {
        let (status, error_msg, code) = match self {
            ObjectStorageError::ConfigError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
                ApiErrorCode::Status(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
            ),
            ObjectStorageError::InvalidInput(msg) => (
                StatusCode::BAD_REQUEST,
                msg,
                ApiErrorCode::Status(StatusCode::BAD_REQUEST.as_u16()),
            ),
            ObjectStorageError::StorageError(msg) => (
                StatusCode::BAD_GATEWAY,
                msg,
                ApiErrorCode::Status(StatusCode::BAD_GATEWAY.as_u16()),
            ),
            ObjectStorageError::UploadError(msg) => (
                StatusCode::BAD_REQUEST,
                msg,
                ApiErrorCode::Status(StatusCode::BAD_REQUEST.as_u16()),
            ),
            ObjectStorageError::DownloadError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
                ApiErrorCode::Status(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
            ),
            ObjectStorageError::DeleteError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                msg,
                ApiErrorCode::Status(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
            ),
            ObjectStorageError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                msg,
                ApiErrorCode::Status(StatusCode::NOT_FOUND.as_u16()),
            ),
            ObjectStorageError::ObjectConflict(msg) => (
                StatusCode::CONFLICT,
                msg,
                ApiErrorCode::Kind("object_conflict".to_string()),
            ),
        };

        let body: ApiResponse<()> = ApiResponse {
            success: false,
            data: None,
            error: Some(error_msg),
            code: Some(code),
        };

        (status, Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ObjectStorageError>;
