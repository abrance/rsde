use std::sync::Arc;

use axum::{
    Router,
    body::to_bytes,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, patch, put},
};
use config::datalink_engine::{DataLinkEngineBackend, DataLinkEngineConfig};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, DataLinkBundle, DataLinkError, DataLinkListFilter,
    DataLinkService, DataLinkStatus, PaginatedResult, PaginationParams, SetDataLinkStatus,
    StorageType, bootstrap,
    storage::{memory::MemoryDataLinkRepository, mysql::MysqlDataLinkRepository},
};
use query_engine::InMemoryHeartbeatStore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

type ApiResult<T> = (StatusCode, Json<ApiResponse<T>>);
const MAX_JSON_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct DataLinkState {
    service: Arc<DataLinkServiceRuntime>,
}

enum DataLinkServiceRuntime {
    Memory(DataLinkService<MemoryDataLinkRepository>),
    Mysql(DataLinkService<MysqlDataLinkRepository>),
}

#[derive(Clone)]
pub struct SharedMemoryRuntime {
    pub datalink_service: DataLinkService<MemoryDataLinkRepository>,
    pub heartbeat_store: InMemoryHeartbeatStore,
}

impl SharedMemoryRuntime {
    pub fn new() -> Self {
        Self {
            datalink_service: DataLinkService::new(MemoryDataLinkRepository::new()),
            heartbeat_store: InMemoryHeartbeatStore::new(),
        }
    }
}

impl Default for SharedMemoryRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLinkServiceRuntime {
    fn apply_data_link(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        match self {
            Self::Memory(service) => service.apply_data_link(spec, options),
            Self::Mysql(service) => service.apply_data_link(spec, options),
        }
    }

    fn get_data_link(&self, data_link_id: &str) -> datalink_engine::error::Result<DataLinkBundle> {
        match self {
            Self::Memory(service) => service.get_data_link(data_link_id),
            Self::Mysql(service) => service.get_data_link(data_link_id),
        }
    }

    fn get_data_link_by_result_table_name(
        &self,
        result_table_name: &str,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        match self {
            Self::Memory(service) => service.get_data_link_by_result_table_name(result_table_name),
            Self::Mysql(service) => service.get_data_link_by_result_table_name(result_table_name),
        }
    }

    fn list_data_links(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> datalink_engine::error::Result<PaginatedResult<DataLinkBundle>> {
        match self {
            Self::Memory(service) => service.list_data_links(filter, pagination),
            Self::Mysql(service) => service.list_data_links(filter, pagination),
        }
    }

    fn set_data_link_status(
        &self,
        data_link_id: &str,
        request: SetDataLinkStatus,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        match self {
            Self::Memory(service) => service.set_data_link_status(data_link_id, request),
            Self::Mysql(service) => service.set_data_link_status(data_link_id, request),
        }
    }
}

#[derive(Debug, Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T>
where
    T: Serialize,
{
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ApiError>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ListDataLinksQuery {
    domain: Option<String>,
    owner_service: Option<String>,
    data_type: Option<datalink_engine::DataType>,
    status: Option<DataLinkStatus>,
    storage_type: Option<StorageType>,
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_page_size")]
    page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

pub fn create_routes(config: DataLinkEngineConfig) -> anyhow::Result<Router> {
    let state = build_state(config, None)?;
    Ok(build_router(state))
}

pub fn create_routes_with_shared_memory(
    config: DataLinkEngineConfig,
    shared: SharedMemoryRuntime,
) -> anyhow::Result<Router> {
    let state = build_state(config, Some(shared))?;
    Ok(build_router(state))
}

fn build_state(
    config: DataLinkEngineConfig,
    shared: Option<SharedMemoryRuntime>,
) -> anyhow::Result<DataLinkState> {
    let service = match config.backend {
        DataLinkEngineBackend::Memory => match shared {
            Some(shared) => DataLinkServiceRuntime::Memory(shared.datalink_service),
            None => DataLinkServiceRuntime::Memory(bootstrap::build_memory_service()),
        },
        DataLinkEngineBackend::Mysql => {
            let mysql_config = config
                .mysql
                .ok_or_else(|| anyhow::anyhow!("mysql config is required when backend=mysql"))?;
            DataLinkServiceRuntime::Mysql(bootstrap::build_mysql_service(mysql_config)?)
        }
    };

    Ok(DataLinkState {
        service: Arc::new(service),
    })
}

fn build_router(state: DataLinkState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/datalinks:apply", put(apply_data_link))
        .route("/datalinks", get(list_data_links))
        .route("/datalinks/:id", get(get_data_link))
        .route(
            "/datalinks/by-result-table/:result_table_name",
            get(get_by_result_table_name),
        )
        .route("/datalinks/:id/status", patch(patch_data_link_status))
        .with_state(state)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "datalink-engine-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn apply_data_link(
    State(state): State<DataLinkState>,
    headers: HeaderMap,
    request: Request,
) -> ApiResult<DataLinkBundle> {
    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let payload = match parse_json_body::<ApplyDataLinkSpec>(request).await {
        Ok(payload) => payload,
        Err(err) => return err_response(err),
    };

    match state
        .service
        .apply_data_link(payload, ApplyDataLinkOptions { idempotency_key })
    {
        Ok(bundle) => (StatusCode::OK, Json(ApiResponse::ok(bundle))),
        Err(err) => err_response(err),
    }
}

async fn get_data_link(
    State(state): State<DataLinkState>,
    Path(data_link_id): Path<String>,
) -> ApiResult<DataLinkBundle> {
    match state.service.get_data_link(&data_link_id) {
        Ok(bundle) => (StatusCode::OK, Json(ApiResponse::ok(bundle))),
        Err(err) => err_response(err),
    }
}

async fn get_by_result_table_name(
    State(state): State<DataLinkState>,
    Path(result_table_name): Path<String>,
) -> ApiResult<DataLinkBundle> {
    match state
        .service
        .get_data_link_by_result_table_name(&result_table_name)
    {
        Ok(bundle) => (StatusCode::OK, Json(ApiResponse::ok(bundle))),
        Err(err) => err_response(err),
    }
}

async fn list_data_links(
    State(state): State<DataLinkState>,
    request: Request,
) -> ApiResult<PaginatedResult<DataLinkBundle>> {
    let query = match parse_query::<ListDataLinksQuery>(&request) {
        Ok(query) => query,
        Err(err) => return err_response(err),
    };

    let filter = DataLinkListFilter {
        domain: query.domain,
        owner_service: query.owner_service,
        data_type: query.data_type,
        status: query.status,
        storage_type: query.storage_type,
    };

    let pagination = PaginationParams::new(query.page, query.page_size);

    match state.service.list_data_links(filter, pagination) {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::ok(result))),
        Err(err) => err_response(err),
    }
}

async fn patch_data_link_status(
    State(state): State<DataLinkState>,
    Path(data_link_id): Path<String>,
    request: Request,
) -> ApiResult<DataLinkBundle> {
    let payload = match parse_json_body::<SetDataLinkStatus>(request).await {
        Ok(payload) => payload,
        Err(err) => return err_response(err),
    };

    match state.service.set_data_link_status(&data_link_id, payload) {
        Ok(bundle) => (StatusCode::OK, Json(ApiResponse::ok(bundle))),
        Err(err) => err_response(err),
    }
}

async fn parse_json_body<T>(request: Request) -> Result<T, DataLinkError>
where
    T: DeserializeOwned,
{
    let (_parts, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_JSON_BODY_BYTES)
        .await
        .map_err(|err| DataLinkError::InvalidArgument(format!("invalid request body: {err}")))?;
    serde_json::from_slice::<T>(&bytes)
        .map_err(|err| DataLinkError::InvalidArgument(format!("invalid request body: {err}")))
}

fn parse_query<T>(request: &Request) -> Result<T, DataLinkError>
where
    T: DeserializeOwned,
{
    match Query::<T>::try_from_uri(request.uri()) {
        Ok(Query(value)) => Ok(value),
        Err(err) => Err(DataLinkError::InvalidArgument(format!(
            "invalid query params: {err}"
        ))),
    }
}

fn err_response<T>(err: DataLinkError) -> ApiResult<T>
where
    T: Serialize,
{
    let (status, code) = match &err {
        DataLinkError::InvalidArgument(_) => (StatusCode::BAD_REQUEST, "DL_INVALID_ARGUMENT"),
        DataLinkError::EnumNotSupported(_) => (StatusCode::BAD_REQUEST, "DL_ENUM_NOT_SUPPORTED"),
        DataLinkError::ResultTableNameConflict(_) => {
            (StatusCode::CONFLICT, "DL_RESULT_TABLE_NAME_CONFLICT")
        }
        DataLinkError::EtlPipelineInvalid(_) => {
            (StatusCode::BAD_REQUEST, "DL_ETL_PIPELINE_INVALID")
        }
        DataLinkError::EtlModeNotSupported(_) => {
            (StatusCode::BAD_REQUEST, "DL_ETL_MODE_NOT_SUPPORTED")
        }
        DataLinkError::NotFound(_) => (StatusCode::NOT_FOUND, "DL_NOT_FOUND"),
        DataLinkError::StatusTransitionInvalid { .. } => {
            (StatusCode::BAD_REQUEST, "DL_STATUS_TRANSITION_INVALID")
        }
        DataLinkError::StatusMessageRequired => {
            (StatusCode::BAD_REQUEST, "DL_STATUS_MESSAGE_REQUIRED")
        }
        DataLinkError::StatusMessageInvalid(_) => {
            (StatusCode::BAD_REQUEST, "DL_STATUS_MESSAGE_INVALID")
        }
        DataLinkError::IdempotencyConflict(_) => (StatusCode::CONFLICT, "DL_IDEMPOTENCY_CONFLICT"),
        DataLinkError::BackendNotSupported(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DL_BACKEND_NOT_SUPPORTED",
        ),
        DataLinkError::Repository(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DL_REPOSITORY_ERROR"),
    };

    (status, Json(ApiResponse::err(code, err.to_string())))
}
