use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
};
use chrono::{TimeDelta, Utc};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkBundle, DataLinkStatus,
    DataSourceInput, DataType, EtlMode, EtlPipelineInput, ResultTableInput, StorageType,
};
use nodemanage::{
    AgentRegistration, CreateNode, InstallNodeRequest, InstallPlugin, MemoryNodeRepository,
    MySqlNodeRepository, Node, NodeManageError, NodeManager, NodeStatus, NoopRsAgentInstaller,
    PaginatedResult, PaginationParams, RemoteExecutor, RepositoryRegistrationWaiter,
    ShellRemoteExecutor, SshRsAgentInstaller, UpdateNode,
};
use query_engine::{InMemoryHeartbeatStore, QueryEngine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::datalink_engine::SharedMemoryRuntime;

type MemoryManager = NodeManager<MemoryNodeRepository, NoopRsAgentInstaller>;
type MysqlManager = NodeManager<MySqlNodeRepository, SshRsAgentInstaller>;
type MemoryQueryEngine =
    QueryEngine<datalink_engine::storage::memory::MemoryDataLinkRepository, InMemoryHeartbeatStore>;

#[derive(Clone)]
enum AppNodeManager {
    Memory(MemoryManager),
    Mysql(MysqlManager),
}

impl AppNodeManager {
    async fn create(&self, req: CreateNode) -> Result<Node, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.create(req).await,
            Self::Mysql(manager) => manager.create(req).await,
        }
    }

    async fn get(&self, id: &str) -> Result<Option<Node>, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.get(id).await,
            Self::Mysql(manager) => manager.get(id).await,
        }
    }

    async fn list(
        &self,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<Node>, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.list(pagination).await,
            Self::Mysql(manager) => manager.list(pagination).await,
        }
    }

    async fn update(&self, id: &str, req: UpdateNode) -> Result<Node, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.update(id, req).await,
            Self::Mysql(manager) => manager.update(id, req).await,
        }
    }

    async fn delete(&self, id: &str) -> Result<bool, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.delete(id).await,
            Self::Mysql(manager) => manager.delete(id).await,
        }
    }

    async fn heartbeat(&self, id: &str) -> Result<Node, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.heartbeat(id).await,
            Self::Mysql(manager) => manager.heartbeat(id).await,
        }
    }

    async fn update_status(&self, id: &str, status: NodeStatus) -> Result<Node, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.update_status(id, status).await,
            Self::Mysql(manager) => manager.update_status(id, status).await,
        }
    }

    async fn install_node(
        &self,
        req: InstallNodeRequest,
    ) -> Result<nodemanage::InstallNodeResult, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.install_node(req).await,
            Self::Mysql(manager) => manager.install_node(req).await,
        }
    }

    async fn register_agent(&self, req: AgentRegistration) -> Result<Node, NodeManageError> {
        match self {
            Self::Memory(manager) => manager.register_agent(req).await,
            Self::Mysql(manager) => manager.register_agent(req).await,
        }
    }
}

#[derive(Clone)]
pub struct NodeManageState {
    manager: AppNodeManager,
    config: config::nodemanage::NodeManageConfig,
    query_engine: Option<MemoryQueryEngine>,
    heartbeat_data_link_id: Option<String>,
}

impl NodeManageState {
    pub async fn new(config: config::nodemanage::NodeManageConfig) -> anyhow::Result<Self> {
        Self::new_with_shared_memory(config, None).await
    }

    pub async fn new_with_shared_memory(
        config: config::nodemanage::NodeManageConfig,
        shared: Option<SharedMemoryRuntime>,
    ) -> anyhow::Result<Self> {
        let heartbeat_data_link_id = shared
            .as_ref()
            .map(|shared| bootstrap_heartbeat_datalink(shared, &config))
            .transpose()?
            .map(|bundle| bundle.data_link.data_link_id);
        let (manager, query_engine) = assemble_manager(&config, shared).await?;
        Ok(Self {
            manager,
            config,
            query_engine,
            heartbeat_data_link_id,
        })
    }
}

fn heartbeat_apply_spec(config: &config::nodemanage::NodeManageConfig) -> ApplyDataLinkSpec {
    ApplyDataLinkSpec {
        name: "nodemanage_node_heartbeat".to_string(),
        description: Some("shared heartbeat datalink for all managed nodes".to_string()),
        domain: "nodemanage".to_string(),
        owner_service: "nodemanage".to_string(),
        data_type: DataType::Metric,
        status: DataLinkStatus::Active,
        status_message: None,
        datasource: DataSourceInput {
            producer: "rsagent".to_string(),
            data_type: DataType::Metric,
            collect_method: CollectMethod::Agent,
            protocol: Some("http".to_string()),
            interval_seconds: Some(config.heartbeat.interval_seconds),
            labels: HashMap::from([
                ("domain".to_string(), "nodemanage".to_string()),
                ("link_purpose".to_string(), "node_heartbeat".to_string()),
            ]),
            dimension_keys: vec![
                "node_id".to_string(),
                "agent_id".to_string(),
                "node_ip".to_string(),
            ],
            auth_ref: None,
            config: HashMap::new(),
        },
        etl_pipeline: EtlPipelineInput {
            mode: EtlMode::Passthrough,
            config: HashMap::new(),
        },
        result_table: ResultTableInput {
            result_table_name: config.heartbeat.result_table_name.clone(),
            storage_type: StorageType::Victoriametrics,
            storage_cluster: Some(config.heartbeat.storage_cluster.clone()),
            database: None,
            table_name: None,
            metric_name: Some(config.heartbeat.metric_name.clone()),
            query_template: Some(config.heartbeat.query_template.clone()),
            schema: HashMap::from([
                ("timestamp".to_string(), "datetime".to_string()),
                ("node_id".to_string(), "string".to_string()),
                ("agent_id".to_string(), "string".to_string()),
                ("node_ip".to_string(), "string".to_string()),
            ]),
            retention_days: Some(config.heartbeat.retention_days),
        },
    }
}

fn bootstrap_heartbeat_datalink(
    shared: &SharedMemoryRuntime,
    config: &config::nodemanage::NodeManageConfig,
) -> anyhow::Result<DataLinkBundle> {
    shared
        .datalink_service
        .apply_data_link(
            heartbeat_apply_spec(config),
            ApplyDataLinkOptions {
                idempotency_key: Some("nodemanage-bootstrap-heartbeat-datalink".to_string()),
            },
        )
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

fn map_install_plugins(plugins: &[config::nodemanage::InstallPluginConfig]) -> Vec<InstallPlugin> {
    plugins
        .iter()
        .map(|plugin| InstallPlugin {
            name: plugin.name.clone(),
            version: plugin.version.clone(),
            package_url: plugin.package_url.clone(),
        })
        .collect()
}

pub fn apply_install_request_defaults(
    config: &config::nodemanage::NodeManageConfig,
    mut request: InstallNodeRequest,
) -> InstallNodeRequest {
    if request.rsagent_package_url.is_empty() {
        if let Some(url) = &config.rsagent_package_url {
            request.rsagent_package_url = url.clone();
        }
    }

    if request.install_root.is_empty() {
        request.install_root = config.install_root.clone();
    }

    if request.register_callback_url.is_empty() {
        request.register_callback_url = config.register_callback_url.clone();
    }

    if request.plugins.is_empty() {
        request.plugins = map_install_plugins(&config.install_plugins);
    }

    request
}

async fn assemble_manager(
    config: &config::nodemanage::NodeManageConfig,
    shared: Option<SharedMemoryRuntime>,
) -> anyhow::Result<(AppNodeManager, Option<MemoryQueryEngine>)> {
    if let Some(mysql) = &config.mysql {
        let repository = MySqlNodeRepository::new(mysql.clone(), config.table_prefix.clone())
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let waiter = RepositoryRegistrationWaiter::new(repository.clone(), 1);
        let executor: Arc<dyn RemoteExecutor> = Arc::new(ShellRemoteExecutor);
        let installer = SshRsAgentInstaller::new(
            executor,
            Arc::new(waiter),
            map_install_plugins(&config.install_plugins),
            config.register_wait_timeout_secs,
        );
        Ok((
            AppNodeManager::Mysql(NodeManager::new(repository, installer)),
            None,
        ))
    } else {
        let query_engine =
            shared.map(|shared| QueryEngine::new(shared.datalink_service, shared.heartbeat_store));
        Ok((
            AppNodeManager::Memory(NodeManager::new(
                MemoryNodeRepository::default(),
                NoopRsAgentInstaller,
            )),
            query_engine,
        ))
    }
}

#[derive(Debug, Serialize)]
pub struct NodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListNodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PaginatedResult<Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstallNodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<nodemanage::InstallNodeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListNodeQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

fn node_error(error: NodeManageError) -> (StatusCode, Json<NodeResponse>) {
    let status = match error {
        NodeManageError::NotFound(_) => StatusCode::NOT_FOUND,
        NodeManageError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        NodeManageError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(NodeResponse {
            success: false,
            data: None,
            error: Some(error.to_string()),
        }),
    )
}

async fn health_check(State(state): State<NodeManageState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "nodemanage-api",
        "version": env!("CARGO_PKG_VERSION"),
        "table_prefix": state.config.table_prefix,
    }))
}

async fn create_node(
    State(state): State<NodeManageState>,
    Json(req): Json<CreateNode>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    state
        .manager
        .create(req)
        .await
        .map(|node| {
            Json(NodeResponse {
                success: true,
                data: Some(node),
                error: None,
            })
        })
        .map_err(node_error)
}

async fn list_nodes(
    State(state): State<NodeManageState>,
    Query(query): Query<ListNodeQuery>,
) -> Result<Json<ListNodeResponse>, (StatusCode, Json<ListNodeResponse>)> {
    state
        .manager
        .list(PaginationParams::new(query.page, query.page_size))
        .await
        .map(|nodes| {
            Json(ListNodeResponse {
                success: true,
                data: Some(nodes),
                error: None,
            })
        })
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ListNodeResponse {
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                }),
            )
        })
}

async fn get_node(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    match state.manager.get(&id).await.map_err(node_error)? {
        Some(node) => Ok(Json(NodeResponse {
            success: true,
            data: Some(node),
            error: None,
        })),
        None => Err(node_error(NodeManageError::NotFound(id))),
    }
}

async fn update_node(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNode>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    state
        .manager
        .update(&id, req)
        .await
        .map(|node| {
            Json(NodeResponse {
                success: true,
                data: Some(node),
                error: None,
            })
        })
        .map_err(node_error)
}

async fn delete_node(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    match state.manager.delete(&id).await.map_err(node_error)? {
        true => Ok(Json(NodeResponse {
            success: true,
            data: None,
            error: None,
        })),
        false => Err(node_error(NodeManageError::NotFound(id))),
    }
}

async fn heartbeat_node(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    state
        .manager
        .heartbeat(&id)
        .await
        .map(|node| {
            Json(NodeResponse {
                success: true,
                data: Some(node),
                error: None,
            })
        })
        .map_err(node_error)
}

async fn update_status(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStatusRequest>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    let status = NodeStatus::parse(&req.status)
        .ok_or_else(|| node_error(NodeManageError::InvalidInput(req.status)))?;
    state
        .manager
        .update_status(&id, status)
        .await
        .map(|node| {
            Json(NodeResponse {
                success: true,
                data: Some(node),
                error: None,
            })
        })
        .map_err(node_error)
}

async fn refresh_status(
    State(state): State<NodeManageState>,
    Path(id): Path<String>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    let query_engine = state.query_engine.as_ref().ok_or_else(|| {
        node_error(NodeManageError::InvalidInput(
            "query engine unavailable".to_string(),
        ))
    })?;

    match &state.manager {
        AppNodeManager::Memory(manager) => {
            let heartbeat_data_link_id =
                state.heartbeat_data_link_id.as_ref().ok_or_else(|| {
                    node_error(NodeManageError::InvalidInput(
                        "heartbeat datalink unavailable".to_string(),
                    ))
                })?;

            manager
                .refresh_status_from_query(
                    &id,
                    query_engine,
                    heartbeat_data_link_id,
                    Utc::now(),
                    TimeDelta::seconds(state.config.heartbeat.status_window_secs as i64),
                )
                .await
                .map(|node| {
                    Json(NodeResponse {
                        success: true,
                        data: Some(node),
                        error: None,
                    })
                })
                .map_err(node_error)
        }
        AppNodeManager::Mysql(_) => Err(node_error(NodeManageError::InvalidInput(
            "query refresh not supported for mysql backend yet".to_string(),
        ))),
    }
}

async fn install_node(
    State(state): State<NodeManageState>,
    Json(mut req): Json<InstallNodeRequest>,
) -> Result<Json<InstallNodeResponse>, (StatusCode, Json<InstallNodeResponse>)> {
    req = apply_install_request_defaults(&state.config, req);

    state
        .manager
        .install_node(req)
        .await
        .map(|result| {
            Json(InstallNodeResponse {
                success: true,
                data: Some(result),
                error: None,
            })
        })
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(InstallNodeResponse {
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                }),
            )
        })
}

async fn register_agent(
    State(state): State<NodeManageState>,
    Json(req): Json<AgentRegistration>,
) -> Result<Json<NodeResponse>, (StatusCode, Json<NodeResponse>)> {
    state
        .manager
        .register_agent(req)
        .await
        .map(|node| {
            Json(NodeResponse {
                success: true,
                data: Some(node),
                error: None,
            })
        })
        .map_err(node_error)
}

pub async fn create_routes(config: config::nodemanage::NodeManageConfig) -> anyhow::Result<Router> {
    let state = NodeManageState::new(config).await?;

    Ok(build_router(state))
}

pub async fn create_routes_with_shared_memory(
    config: config::nodemanage::NodeManageConfig,
    shared: SharedMemoryRuntime,
) -> anyhow::Result<Router> {
    let state = NodeManageState::new_with_shared_memory(config, Some(shared)).await?;

    Ok(build_router(state))
}

fn build_router(state: NodeManageState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/node", post(create_node))
        .route("/node", get(list_nodes))
        .route("/node/:id", get(get_node))
        .route("/node/:id", put(update_node))
        .route("/node/:id", delete(delete_node))
        .route("/node/:id/heartbeat", post(heartbeat_node))
        .route("/node/:id/status", patch(update_status))
        .route("/node/:id/status/refresh", post(refresh_status))
        .route("/install", post(install_node))
        .route("/agent/register", post(register_agent))
        .with_state(state)
}
