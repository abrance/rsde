pub mod dto;
pub mod error;
pub mod qiniu;
pub mod service;

use axum::{
    Router,
    extract::{Query, State},
    response::Json,
    routing::{get, post},
};
use dto::{
    ApiResponse, CreateDirectoryRequest, CreateDirectoryResponse, CreateUploadTokenRequest,
    CreateUploadTokenResponse, DeleteObjectRequest, DeleteObjectResponse, DeleteObjectsRequest,
    DeleteObjectsResponse, DownloadUrlQuery, DownloadUrlResponse, HealthResponse, ListObjectsQuery,
    ListObjectsResponse, MoveObjectRequest, MoveObjectResponse, ObjectDetailQuery,
    ObjectDetailResponse,
};
use error::Result;
use qiniu::QiniuObjectStorageBackend;
use service::{ObjectStorageBackend, ObjectStorageService};
use std::sync::Arc;

#[derive(Clone)]
pub struct ObjectStorageState {
    service: Arc<ObjectStorageService>,
}

async fn health(
    State(state): State<ObjectStorageState>,
) -> Result<Json<ApiResponse<HealthResponse>>> {
    let response = state.service.health_check().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: Some(response),
        error: None,
        code: None,
    }))
}

async fn list_objects(
    State(state): State<ObjectStorageState>,
    Query(query): Query<ListObjectsQuery>,
) -> Result<Json<ApiResponse<ListObjectsResponse>>> {
    let response = state
        .service
        .list_objects(query.prefix, query.marker, query.limit)
        .await?;
    Ok(Json(success_response(response)))
}

async fn object_detail(
    State(state): State<ObjectStorageState>,
    Query(query): Query<ObjectDetailQuery>,
) -> Result<Json<ApiResponse<ObjectDetailResponse>>> {
    let response = state.service.get_object_detail(&query.key).await?;
    Ok(Json(success_response(response)))
}

async fn create_directory(
    State(state): State<ObjectStorageState>,
    Json(request): Json<CreateDirectoryRequest>,
) -> Result<Json<ApiResponse<CreateDirectoryResponse>>> {
    let response = state
        .service
        .create_directory(request.prefix.as_deref(), &request.name)
        .await?;
    Ok(Json(success_response(response)))
}

async fn move_object(
    State(state): State<ObjectStorageState>,
    Json(request): Json<MoveObjectRequest>,
) -> Result<Json<ApiResponse<MoveObjectResponse>>> {
    let response = state
        .service
        .move_object(&request.from_key, &request.to_key)
        .await?;
    Ok(Json(success_response(response)))
}

async fn delete_object(
    State(state): State<ObjectStorageState>,
    Json(request): Json<DeleteObjectRequest>,
) -> Result<Json<ApiResponse<DeleteObjectResponse>>> {
    let response = state.service.delete_object(&request.key).await?;
    Ok(Json(success_response(response)))
}

async fn delete_objects(
    State(state): State<ObjectStorageState>,
    Json(request): Json<DeleteObjectsRequest>,
) -> Result<Json<ApiResponse<DeleteObjectsResponse>>> {
    let response = state.service.delete_objects(request.keys).await?;
    Ok(Json(success_response(response)))
}

async fn create_upload_token(
    State(state): State<ObjectStorageState>,
    Json(request): Json<CreateUploadTokenRequest>,
) -> Result<Json<ApiResponse<CreateUploadTokenResponse>>> {
    let response = state
        .service
        .create_upload_token(request.prefix.as_deref(), &request.filename)?;
    Ok(Json(success_response(response)))
}

async fn create_download_url(
    State(state): State<ObjectStorageState>,
    Query(query): Query<DownloadUrlQuery>,
) -> Result<Json<ApiResponse<DownloadUrlResponse>>> {
    let response = state.service.create_download_url(&query.key)?;
    Ok(Json(success_response(response)))
}

fn success_response<T: serde::Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse {
        success: true,
        data: Some(data),
        error: None,
        code: None,
    }
}

pub fn create_routes(config: config::object_storage::ObjectStorageConfig) -> Router {
    let backend = Arc::new(QiniuObjectStorageBackend::new(&config));
    create_routes_with_backend(config, backend)
}

pub fn create_routes_with_backend(
    config: config::object_storage::ObjectStorageConfig,
    backend: Arc<dyn ObjectStorageBackend>,
) -> Router {
    let service = ObjectStorageService::new(config, backend);
    let state = ObjectStorageState {
        service: Arc::new(service),
    };

    Router::new()
        .route("/health", get(health))
        .route("/objects", get(list_objects))
        .route("/objects/detail", get(object_detail))
        .route("/objects/move", post(move_object))
        .route("/objects/delete", post(delete_object))
        .route("/objects/delete-batch", post(delete_objects))
        .route("/directories", post(create_directory))
        .route("/upload-token", post(create_upload_token))
        .route("/download-url", get(create_download_url))
        .with_state(state)
}
