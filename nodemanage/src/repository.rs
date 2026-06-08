use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use config::mysql::MysqlConfig;
use mysql_async::{Pool, Row, params, prelude::*};
use tokio::sync::Mutex;

use crate::{
    BindingState, InstallTaskState, InstallTaskStep, Node, NodeAgentBinding, NodeInstallTask,
    NodeManageError, NodeStatus, PaginatedResult, PaginationParams, Result,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct InstallTaskRequestContext {
    request_host: Option<String>,
    request_ssh_port: Option<u16>,
    request_username: Option<String>,
    request_rsagent_package_url: Option<String>,
    request_install_root: Option<String>,
    request_labels: Vec<String>,
    request_plugin_names: Vec<String>,
}

#[async_trait]
pub trait NodeRepository: Clone + Send + Sync + 'static {
    async fn create(&self, node: Node) -> Result<Node>;
    async fn get(&self, id: &str) -> Result<Option<Node>>;
    async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>>;
    async fn update(&self, node: Node) -> Result<Node>;
    async fn delete(&self, id: &str) -> Result<bool>;
    async fn create_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask>;
    async fn update_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask>;
    async fn get_install_task(&self, install_task_id: &str) -> Result<Option<NodeInstallTask>>;
    async fn latest_install_task_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeInstallTask>>;
    async fn upsert_agent_binding(&self, binding: NodeAgentBinding) -> Result<NodeAgentBinding>;
    async fn agent_binding_by_agent_id(&self, agent_id: &str) -> Result<Option<NodeAgentBinding>>;
    async fn bound_agent_binding_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeAgentBinding>>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryNodeRepository {
    nodes: Arc<Mutex<HashMap<String, Node>>>,
    install_tasks: Arc<Mutex<HashMap<String, NodeInstallTask>>>,
    bindings: Arc<Mutex<HashMap<String, NodeAgentBinding>>>,
}

#[async_trait]
impl NodeRepository for MemoryNodeRepository {
    async fn create(&self, node: Node) -> Result<Node> {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(node.id.clone(), node.clone());
        Ok(node)
    }

    async fn get(&self, id: &str) -> Result<Option<Node>> {
        let nodes = self.nodes.lock().await;
        Ok(nodes.get(id).cloned())
    }

    async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>> {
        let nodes = self.nodes.lock().await;
        let mut items: Vec<_> = nodes.values().cloned().collect();
        items.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let total = items.len() as u64;
        let start = pagination.offset();
        let end = start.saturating_add(pagination.page_size as usize);
        let paged_items = items
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect();

        Ok(PaginatedResult::new(paged_items, total, pagination))
    }

    async fn update(&self, node: Node) -> Result<Node> {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(node.id.clone(), node.clone());
        Ok(node)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let mut nodes = self.nodes.lock().await;
        Ok(nodes.remove(id).is_some())
    }

    async fn create_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask> {
        let mut tasks = self.install_tasks.lock().await;
        tasks.insert(task.install_task_id.clone(), task.clone());
        Ok(task)
    }

    async fn update_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask> {
        let mut tasks = self.install_tasks.lock().await;
        tasks.insert(task.install_task_id.clone(), task.clone());
        Ok(task)
    }

    async fn get_install_task(&self, install_task_id: &str) -> Result<Option<NodeInstallTask>> {
        let tasks = self.install_tasks.lock().await;
        Ok(tasks.get(install_task_id).cloned())
    }

    async fn latest_install_task_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeInstallTask>> {
        let tasks = self.install_tasks.lock().await;
        Ok(tasks
            .values()
            .filter(|task| task.node_id == node_id)
            .cloned()
            .max_by(|left, right| {
                left.started_at
                    .cmp(&right.started_at)
                    .then_with(|| left.install_task_id.cmp(&right.install_task_id))
            }))
    }

    async fn upsert_agent_binding(&self, binding: NodeAgentBinding) -> Result<NodeAgentBinding> {
        let mut bindings = self.bindings.lock().await;
        bindings.insert(binding.agent_id.clone(), binding.clone());
        Ok(binding)
    }

    async fn agent_binding_by_agent_id(&self, agent_id: &str) -> Result<Option<NodeAgentBinding>> {
        let bindings = self.bindings.lock().await;
        Ok(bindings.get(agent_id).cloned())
    }

    async fn bound_agent_binding_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeAgentBinding>> {
        let bindings = self.bindings.lock().await;
        Ok(bindings
            .values()
            .find(|binding| {
                binding.node_id == node_id && binding.binding_state == BindingState::Bound
            })
            .cloned())
    }
}

#[derive(Debug, Clone)]
pub struct MySqlNodeRepository {
    pool: Pool,
    table_name: String,
    install_task_table_name: String,
    binding_table_name: String,
}

impl MySqlNodeRepository {
    fn install_task_request_context_json(task: &NodeInstallTask) -> Result<String> {
        serde_json::to_string(&InstallTaskRequestContext {
            request_host: task.request_host.clone(),
            request_ssh_port: task.request_ssh_port,
            request_username: task.request_username.clone(),
            request_rsagent_package_url: task.request_rsagent_package_url.clone(),
            request_install_root: task.request_install_root.clone(),
            request_labels: task.request_labels.clone(),
            request_plugin_names: task.request_plugin_names.clone(),
        })
        .map_err(Into::into)
    }

    pub async fn new(config: MysqlConfig, table_prefix: String) -> Result<Self> {
        let opts = mysql_async::Opts::from_url(&config.connection_url())
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        let repository = Self {
            pool: Pool::new(opts),
            table_name: format!("{table_prefix}nodes"),
            install_task_table_name: format!("{table_prefix}install_tasks"),
            binding_table_name: format!("{table_prefix}agent_bindings"),
        };
        repository.init_table().await?;
        Ok(repository)
    }

    async fn init_table(&self) -> Result<()> {
        let mut conn = self.connection().await?;
        let create_nodes_table_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS `{}` (
                `id` VARCHAR(36) NOT NULL PRIMARY KEY,
                `name` VARCHAR(255) NOT NULL,
                `endpoint` VARCHAR(1024) NOT NULL,
                `status` VARCHAR(32) NOT NULL,
                `labels` JSON,
                `created_at` DATETIME NOT NULL,
                `updated_at` DATETIME NOT NULL,
                `last_heartbeat_at` DATETIME NULL,
                INDEX `idx_created_at` (`created_at`),
                INDEX `idx_status` (`status`)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci"#,
            self.table_name
        );
        conn.query_drop(create_nodes_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;

        let create_install_tasks_table_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS `{}` (
                `install_task_id` VARCHAR(36) NOT NULL PRIMARY KEY,
                `node_id` VARCHAR(255) NOT NULL,
                `task_state` VARCHAR(32) NOT NULL,
                `current_step` VARCHAR(64) NULL,
                `error_code` VARCHAR(128) NULL,
                `error_message` TEXT NULL,
                `started_at` DATETIME(6) NOT NULL,
                `finished_at` DATETIME(6) NULL,
                `retryable` BOOLEAN NOT NULL,
                `request_context` JSON NULL,
                INDEX `idx_node_started_at` (`node_id`, `started_at`),
                INDEX `idx_task_state` (`task_state`)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci"#,
            self.install_task_table_name
        );
        conn.query_drop(create_install_tasks_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;

        let create_bindings_table_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS `{}` (
                `agent_id` VARCHAR(255) NOT NULL PRIMARY KEY,
                `node_id` VARCHAR(255) NOT NULL,
                `binding_state` VARCHAR(32) NOT NULL,
                `first_registered_at` DATETIME NOT NULL,
                `last_handshake_at` DATETIME NOT NULL,
                `unbind_reason` TEXT NULL,
                INDEX `idx_node_binding_state` (`node_id`, `binding_state`),
                INDEX `idx_last_handshake_at` (`last_handshake_at`)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci"#,
            self.binding_table_name
        );
        conn.query_drop(create_bindings_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(())
    }

    async fn connection(&self) -> Result<mysql_async::Conn> {
        self.pool
            .get_conn()
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))
    }

    fn status_as_str(status: &NodeStatus) -> &'static str {
        match status {
            NodeStatus::Online => "online",
            NodeStatus::Offline => "offline",
            NodeStatus::Maintenance => "maintenance",
        }
    }

    fn row_to_node(&self, row: Row) -> Result<Node> {
        let (id, name, endpoint, status, labels_json, created_at, updated_at, last_heartbeat_at): (
            String,
            String,
            String,
            String,
            Option<String>,
            NaiveDateTime,
            NaiveDateTime,
            Option<NaiveDateTime>,
        ) = mysql_async::from_row(row);

        Ok(Node {
            id,
            name,
            endpoint,
            status: NodeStatus::parse(&status).ok_or_else(|| {
                NodeManageError::Storage(format!("unknown node status: {status}"))
            })?,
            labels: labels_json
                .map(|value| serde_json::from_str::<Vec<String>>(&value))
                .transpose()?
                .unwrap_or_default(),
            created_at: DateTime::from_naive_utc_and_offset(created_at, Utc),
            updated_at: DateTime::from_naive_utc_and_offset(updated_at, Utc),
            last_heartbeat_at: last_heartbeat_at
                .map(|value| DateTime::from_naive_utc_and_offset(value, Utc)),
        })
    }

    fn binding_state_as_str(binding_state: &BindingState) -> &'static str {
        match binding_state {
            BindingState::Bound => "bound",
            BindingState::Stale => "stale",
            BindingState::Unbound => "unbound",
        }
    }

    fn install_task_state_as_str(task_state: &InstallTaskState) -> &'static str {
        match task_state {
            InstallTaskState::Pending => "pending",
            InstallTaskState::Running => "running",
            InstallTaskState::WaitingRegister => "waiting_register",
            InstallTaskState::Succeeded => "succeeded",
            InstallTaskState::Failed => "failed",
            InstallTaskState::Cancelled => "cancelled",
        }
    }

    fn install_task_step_as_str(step: &InstallTaskStep) -> &'static str {
        match step {
            InstallTaskStep::PrepareInstall => "prepare_install",
            InstallTaskStep::ResolveArtifacts => "resolve_artifacts",
            InstallTaskStep::WriteRuntimeConfig => "write_runtime_config",
            InstallTaskStep::UploadPackage => "upload_package",
            InstallTaskStep::RunInstallScript => "run_install_script",
            InstallTaskStep::StartAgent => "start_agent",
            InstallTaskStep::WaitRegister => "wait_register",
        }
    }

    fn parse_install_task_state(value: &str) -> Result<InstallTaskState> {
        match value {
            "pending" => Ok(InstallTaskState::Pending),
            "running" => Ok(InstallTaskState::Running),
            "waiting_register" => Ok(InstallTaskState::WaitingRegister),
            "succeeded" => Ok(InstallTaskState::Succeeded),
            "failed" => Ok(InstallTaskState::Failed),
            "cancelled" => Ok(InstallTaskState::Cancelled),
            _ => Err(NodeManageError::Storage(format!(
                "unknown install task state: {value}"
            ))),
        }
    }

    fn parse_install_task_step(value: &str) -> Result<InstallTaskStep> {
        match value {
            "prepare_install" => Ok(InstallTaskStep::PrepareInstall),
            "resolve_artifacts" => Ok(InstallTaskStep::ResolveArtifacts),
            "write_runtime_config" => Ok(InstallTaskStep::WriteRuntimeConfig),
            "upload_package" => Ok(InstallTaskStep::UploadPackage),
            "run_install_script" => Ok(InstallTaskStep::RunInstallScript),
            "start_agent" => Ok(InstallTaskStep::StartAgent),
            "wait_register" => Ok(InstallTaskStep::WaitRegister),
            _ => Err(NodeManageError::Storage(format!(
                "unknown install task step: {value}"
            ))),
        }
    }

    fn row_to_install_task(&self, row: Row) -> Result<NodeInstallTask> {
        let (
            install_task_id,
            node_id,
            task_state,
            current_step,
            error_code,
            error_message,
            started_at,
            finished_at,
            retryable,
            request_context,
        ): (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            NaiveDateTime,
            Option<NaiveDateTime>,
            bool,
            Option<String>,
        ) = mysql_async::from_row_opt(row)
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;

        let request_context = request_context
            .map(|value| serde_json::from_str::<InstallTaskRequestContext>(&value))
            .transpose()?;

        Ok(NodeInstallTask {
            install_task_id,
            node_id,
            task_state: Self::parse_install_task_state(&task_state)?,
            current_step: current_step
                .as_deref()
                .map(Self::parse_install_task_step)
                .transpose()?,
            error_code,
            error_message,
            started_at: DateTime::from_naive_utc_and_offset(started_at, Utc),
            finished_at: finished_at.map(|value| DateTime::from_naive_utc_and_offset(value, Utc)),
            retryable,
            request_host: request_context
                .as_ref()
                .and_then(|ctx| ctx.request_host.clone()),
            request_ssh_port: request_context
                .as_ref()
                .and_then(|ctx| ctx.request_ssh_port),
            request_username: request_context
                .as_ref()
                .and_then(|ctx| ctx.request_username.clone()),
            request_rsagent_package_url: request_context
                .as_ref()
                .and_then(|ctx| ctx.request_rsagent_package_url.clone()),
            request_install_root: request_context
                .as_ref()
                .and_then(|ctx| ctx.request_install_root.clone()),
            request_labels: request_context
                .as_ref()
                .map(|ctx| ctx.request_labels.clone())
                .unwrap_or_default(),
            request_plugin_names: request_context
                .as_ref()
                .map(|ctx| ctx.request_plugin_names.clone())
                .unwrap_or_default(),
        })
    }

    fn row_to_binding(&self, row: Row) -> Result<NodeAgentBinding> {
        let (
            agent_id,
            node_id,
            binding_state,
            first_registered_at,
            last_handshake_at,
            unbind_reason,
        ): (
            String,
            String,
            String,
            NaiveDateTime,
            NaiveDateTime,
            Option<String>,
        ) = mysql_async::from_row(row);

        Ok(NodeAgentBinding {
            node_id,
            agent_id,
            binding_state: match binding_state.as_str() {
                "bound" => BindingState::Bound,
                "stale" => BindingState::Stale,
                "unbound" => BindingState::Unbound,
                _ => {
                    return Err(NodeManageError::Storage(format!(
                        "unknown binding state: {binding_state}"
                    )));
                }
            },
            first_registered_at: DateTime::from_naive_utc_and_offset(first_registered_at, Utc),
            last_handshake_at: DateTime::from_naive_utc_and_offset(last_handshake_at, Utc),
            unbind_reason,
        })
    }

    pub async fn reset_for_tests(&self) -> Result<()> {
        let mut conn = self.connection().await?;
        let drop_bindings_table_sql = format!("DROP TABLE IF EXISTS `{}`", self.binding_table_name);
        conn.query_drop(drop_bindings_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        let drop_install_tasks_table_sql =
            format!("DROP TABLE IF EXISTS `{}`", self.install_task_table_name);
        conn.query_drop(drop_install_tasks_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        let drop_nodes_table_sql = format!("DROP TABLE IF EXISTS `{}`", self.table_name);
        conn.query_drop(drop_nodes_table_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        self.init_table().await
    }

    pub async fn list_tables_for_tests(&self) -> Result<Vec<String>> {
        let mut conn = self.connection().await?;
        let like_pattern = self.table_name.replace("nodes", "%");
        let sql = "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name LIKE :pattern ORDER BY table_name";
        let tables: Vec<String> = conn
            .exec_map(
                sql,
                params! { "pattern" => like_pattern },
                |name: String| name,
            )
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(tables)
    }
}

#[async_trait]
impl NodeRepository for MySqlNodeRepository {
    async fn create(&self, node: Node) -> Result<Node> {
        let mut conn = self.connection().await?;
        let insert_sql = format!(
            r#"INSERT INTO `{}` (id, name, endpoint, status, labels, created_at, updated_at, last_heartbeat_at)
               VALUES (:id, :name, :endpoint, :status, :labels, :created_at, :updated_at, :last_heartbeat_at)"#,
            self.table_name
        );
        let labels_json = serde_json::to_string(&node.labels)?;
        conn.exec_drop(
            insert_sql,
            params! {
                "id" => &node.id,
                "name" => &node.name,
                "endpoint" => &node.endpoint,
                "status" => Self::status_as_str(&node.status),
                "labels" => &labels_json,
                "created_at" => node.created_at.naive_utc(),
                "updated_at" => node.updated_at.naive_utc(),
                "last_heartbeat_at" => node.last_heartbeat_at.map(|value| value.naive_utc()),
            },
        )
        .await
        .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(node)
    }

    async fn get(&self, id: &str) -> Result<Option<Node>> {
        let mut conn = self.connection().await?;
        let select_sql = format!(
            "SELECT id, name, endpoint, status, labels, created_at, updated_at, last_heartbeat_at FROM `{}` WHERE id = :id",
            self.table_name
        );
        let row = conn
            .exec_first(select_sql, params! { "id" => id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        row.map(|value| self.row_to_node(value)).transpose()
    }

    async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>> {
        let mut conn = self.connection().await?;
        let count_sql = format!("SELECT COUNT(*) FROM `{}`", self.table_name);
        let total: u64 = conn
            .query_first(count_sql)
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?
            .unwrap_or(0);

        let select_sql = format!(
            "SELECT id, name, endpoint, status, labels, created_at, updated_at, last_heartbeat_at FROM `{}` ORDER BY created_at DESC, id DESC LIMIT :limit OFFSET :offset",
            self.table_name
        );
        let rows: Vec<Row> = conn
            .exec(
                select_sql,
                params! {
                    "limit" => pagination.page_size,
                    "offset" => pagination.offset() as u64,
                },
            )
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;

        let items = rows
            .into_iter()
            .map(|row| self.row_to_node(row))
            .collect::<Result<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total, pagination))
    }

    async fn update(&self, node: Node) -> Result<Node> {
        let mut conn = self.connection().await?;
        let update_sql = format!(
            r#"UPDATE `{}`
               SET name = :name,
                   endpoint = :endpoint,
                   status = :status,
                   labels = :labels,
                   created_at = :created_at,
                   updated_at = :updated_at,
                   last_heartbeat_at = :last_heartbeat_at
               WHERE id = :id"#,
            self.table_name
        );
        let labels_json = serde_json::to_string(&node.labels)?;
        let result = conn
            .exec_iter(
                update_sql,
                params! {
                    "id" => &node.id,
                    "name" => &node.name,
                    "endpoint" => &node.endpoint,
                    "status" => Self::status_as_str(&node.status),
                    "labels" => &labels_json,
                    "created_at" => node.created_at.naive_utc(),
                    "updated_at" => node.updated_at.naive_utc(),
                    "last_heartbeat_at" => node.last_heartbeat_at.map(|value| value.naive_utc()),
                },
            )
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        if result.affected_rows() == 0 {
            return Err(NodeManageError::NotFound(node.id));
        }
        Ok(node)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let mut conn = self.connection().await?;
        let delete_sql = format!("DELETE FROM `{}` WHERE id = :id", self.table_name);
        let result = conn
            .exec_iter(delete_sql, params! { "id" => id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(result.affected_rows() > 0)
    }

    async fn create_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask> {
        let mut conn = self.connection().await?;
        let insert_sql = format!(
            r#"INSERT INTO `{}` (
                install_task_id, node_id, task_state, current_step, error_code, error_message,
                started_at, finished_at, retryable, request_context
               ) VALUES (
                :install_task_id, :node_id, :task_state, :current_step, :error_code, :error_message,
                :started_at, :finished_at, :retryable, :request_context
               )"#,
            self.install_task_table_name
        );
        let request_context = Self::install_task_request_context_json(&task)?;
        conn.exec_drop(
            insert_sql,
            params! {
                "install_task_id" => &task.install_task_id,
                "node_id" => &task.node_id,
                "task_state" => Self::install_task_state_as_str(&task.task_state),
                "current_step" => task.current_step.as_ref().map(|step| Self::install_task_step_as_str(step)),
                "error_code" => &task.error_code,
                "error_message" => &task.error_message,
                "started_at" => task.started_at.naive_utc(),
                "finished_at" => task.finished_at.map(|value| value.naive_utc()),
                "retryable" => task.retryable,
                "request_context" => &request_context,
            },
        )
        .await
        .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(task)
    }

    async fn update_install_task(&self, task: NodeInstallTask) -> Result<NodeInstallTask> {
        let mut conn = self.connection().await?;
        let update_sql = format!(
            r#"UPDATE `{}`
               SET node_id = :node_id,
                   task_state = :task_state,
                   current_step = :current_step,
                   error_code = :error_code,
                   error_message = :error_message,
                   started_at = :started_at,
                   finished_at = :finished_at,
                   retryable = :retryable,
                    request_context = :request_context
                WHERE install_task_id = :install_task_id"#,
            self.install_task_table_name
        );
        let request_context = Self::install_task_request_context_json(&task)?;
        let result = conn
            .exec_iter(
                update_sql,
                params! {
                    "install_task_id" => &task.install_task_id,
                    "node_id" => &task.node_id,
                    "task_state" => Self::install_task_state_as_str(&task.task_state),
                    "current_step" => task.current_step.as_ref().map(|step| Self::install_task_step_as_str(step)),
                    "error_code" => &task.error_code,
                    "error_message" => &task.error_message,
                    "started_at" => task.started_at.naive_utc(),
                    "finished_at" => task.finished_at.map(|value| value.naive_utc()),
                    "retryable" => task.retryable,
                    "request_context" => &request_context,
                },
            )
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        if result.affected_rows() == 0 {
            return Err(NodeManageError::NotFound(task.install_task_id));
        }
        Ok(task)
    }

    async fn get_install_task(&self, install_task_id: &str) -> Result<Option<NodeInstallTask>> {
        let mut conn = self.connection().await?;
        let select_sql = format!(
            "SELECT install_task_id, node_id, task_state, current_step, error_code, error_message, started_at, finished_at, retryable, request_context FROM `{}` WHERE install_task_id = :install_task_id",
            self.install_task_table_name
        );
        let row = conn
            .exec_first(select_sql, params! { "install_task_id" => install_task_id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        row.map(|value| self.row_to_install_task(value)).transpose()
    }

    async fn latest_install_task_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeInstallTask>> {
        let mut conn = self.connection().await?;
        let select_sql = format!(
            "SELECT install_task_id, node_id, task_state, current_step, error_code, error_message, started_at, finished_at, retryable, request_context FROM `{}` WHERE node_id = :node_id ORDER BY started_at DESC, install_task_id DESC LIMIT 1",
            self.install_task_table_name
        );
        let row = conn
            .exec_first(select_sql, params! { "node_id" => node_id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        row.map(|value| self.row_to_install_task(value)).transpose()
    }

    async fn upsert_agent_binding(&self, binding: NodeAgentBinding) -> Result<NodeAgentBinding> {
        let mut conn = self.connection().await?;
        let upsert_sql = format!(
            r#"INSERT INTO `{}` (
                   agent_id, node_id, binding_state, first_registered_at, last_handshake_at, unbind_reason
               ) VALUES (
                   :agent_id, :node_id, :binding_state, :first_registered_at, :last_handshake_at, :unbind_reason
               ) ON DUPLICATE KEY UPDATE
                   node_id = VALUES(node_id),
                   binding_state = VALUES(binding_state),
                   first_registered_at = VALUES(first_registered_at),
                   last_handshake_at = VALUES(last_handshake_at),
                   unbind_reason = VALUES(unbind_reason)"#,
            self.binding_table_name
        );
        conn.exec_drop(
            upsert_sql,
            params! {
                "agent_id" => &binding.agent_id,
                "node_id" => &binding.node_id,
                "binding_state" => Self::binding_state_as_str(&binding.binding_state),
                "first_registered_at" => binding.first_registered_at.naive_utc(),
                "last_handshake_at" => binding.last_handshake_at.naive_utc(),
                "unbind_reason" => &binding.unbind_reason,
            },
        )
        .await
        .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        Ok(binding)
    }

    async fn agent_binding_by_agent_id(&self, agent_id: &str) -> Result<Option<NodeAgentBinding>> {
        let mut conn = self.connection().await?;
        let select_sql = format!(
            "SELECT agent_id, node_id, binding_state, first_registered_at, last_handshake_at, unbind_reason FROM `{}` WHERE agent_id = :agent_id",
            self.binding_table_name
        );
        let row = conn
            .exec_first(select_sql, params! { "agent_id" => agent_id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        row.map(|value| self.row_to_binding(value)).transpose()
    }

    async fn bound_agent_binding_by_node_id(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeAgentBinding>> {
        let mut conn = self.connection().await?;
        let select_sql = format!(
            "SELECT agent_id, node_id, binding_state, first_registered_at, last_handshake_at, unbind_reason FROM `{}` WHERE node_id = :node_id AND binding_state = 'bound' ORDER BY last_handshake_at DESC, agent_id DESC LIMIT 1",
            self.binding_table_name
        );
        let row = conn
            .exec_first(select_sql, params! { "node_id" => node_id })
            .await
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        row.map(|value| self.row_to_binding(value)).transpose()
    }
}
