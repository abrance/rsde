use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
};
use nodemanage::{
    AgentRegistration, CreateNode, InstallNodeRequest, InstallPlugin, MemoryNodeRepository,
    MySqlNodeRepository, Node, NodeManageError, NodeManager, NodeStatus, NoopRsAgentInstaller,
    PaginatedResult, PaginationParams, RemoteExecutor, RepositoryRegistrationWaiter,
    ShellRemoteExecutor, SshRsAgentInstaller, UpdateNode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

type MemoryManager = NodeManager<MemoryNodeRepository, NoopRsAgentInstaller>;
type MysqlManager = NodeManager<MySqlNodeRepository, SshRsAgentInstaller>;

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
}

impl NodeManageState {
    pub async fn new(config: config::nodemanage::NodeManageConfig) -> anyhow::Result<Self> {
        let manager = assemble_manager(&config).await?;
        Ok(Self { manager, config })
    }
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
) -> anyhow::Result<AppNodeManager> {
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
        Ok(AppNodeManager::Mysql(NodeManager::new(
            repository, installer,
        )))
    } else {
        Ok(AppNodeManager::Memory(NodeManager::new(
            MemoryNodeRepository::default(),
            NoopRsAgentInstaller,
        )))
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

    Ok(Router::new()
        .route("/health", get(health_check))
        .route("/node", post(create_node))
        .route("/node", get(list_nodes))
        .route("/node/:id", get(get_node))
        .route("/node/:id", put(update_node))
        .route("/node/:id", delete(delete_node))
        .route("/node/:id/heartbeat", post(heartbeat_node))
        .route("/node/:id/status", patch(update_status))
        .route("/install", post(install_node))
        .route("/agent/register", post(register_agent))
        .with_state(state))
}
