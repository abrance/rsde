use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<ApiErrorCode>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ApiErrorCode {
    Status(u16),
    Kind(String),
}

#[derive(Debug, Deserialize)]
pub struct ListObjectsQuery {
    pub prefix: Option<String>,
    pub marker: Option<String>,
    pub limit: Option<u16>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ObjectPrefix {
    pub key: String,
    pub name: String,
    pub is_directory: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ObjectItem {
    pub key: String,
    pub name: String,
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ListObjectsResponse {
    pub current_prefix: String,
    pub marker: Option<String>,
    pub has_more: bool,
    pub prefixes: Vec<ObjectPrefix>,
    pub items: Vec<ObjectItem>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectDetailQuery {
    pub key: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ObjectDetailResponse {
    pub key: String,
    pub name: String,
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDirectoryRequest {
    pub prefix: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateDirectoryResponse {
    pub key: String,
    pub name: String,
    pub is_directory: bool,
}

#[derive(Debug, Deserialize)]
pub struct MoveObjectRequest {
    pub from_key: String,
    pub to_key: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct MoveObjectResponse {
    pub from_key: String,
    pub to_key: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteObjectRequest {
    pub key: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeleteObjectResponse {
    pub deleted_key: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteObjectsRequest {
    pub keys: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeleteObjectFailure {
    pub key: String,
    pub error: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeleteObjectsResponse {
    pub deleted_keys: Vec<String>,
    pub failed: Vec<DeleteObjectFailure>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUploadTokenRequest {
    pub prefix: Option<String>,
    pub filename: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateUploadTokenResponse {
    pub upload_token: String,
    pub object_key: String,
    pub upload_key: String,
    pub upload_url: String,
    pub expires_at: String,
    pub bucket: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadUrlQuery {
    pub key: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadUrlResponse {
    pub key: String,
    pub download_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}
