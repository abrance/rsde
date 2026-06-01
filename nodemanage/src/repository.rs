use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use config::mysql::MysqlConfig;
use mysql_async::{Pool, Row, params, prelude::*};
use tokio::sync::Mutex;

use crate::{
    BindingState, Node, NodeAgentBinding, NodeManageError, NodeStatus, PaginatedResult,
    PaginationParams, Result,
};

#[async_trait]
pub trait NodeRepository: Clone + Send + Sync + 'static {
    async fn create(&self, node: Node) -> Result<Node>;
    async fn get(&self, id: &str) -> Result<Option<Node>>;
    async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>>;
    async fn update(&self, node: Node) -> Result<Node>;
    async fn delete(&self, id: &str) -> Result<bool>;
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
    binding_table_name: String,
}

impl MySqlNodeRepository {
    pub async fn new(config: MysqlConfig, table_prefix: String) -> Result<Self> {
        let opts = mysql_async::Opts::from_url(&config.connection_url())
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        let repository = Self {
            pool: Pool::new(opts),
            table_name: format!("{table_prefix}nodes"),
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
