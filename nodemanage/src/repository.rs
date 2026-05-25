use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use config::mysql::MysqlConfig;
use mysql_async::{Pool, Row, params, prelude::*};
use tokio::sync::Mutex;

use crate::{Node, NodeManageError, NodeStatus, PaginatedResult, PaginationParams, Result};

#[async_trait]
pub trait NodeRepository: Clone + Send + Sync + 'static {
    async fn create(&self, node: Node) -> Result<Node>;
    async fn get(&self, id: &str) -> Result<Option<Node>>;
    async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>>;
    async fn update(&self, node: Node) -> Result<Node>;
    async fn delete(&self, id: &str) -> Result<bool>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryNodeRepository {
    nodes: Arc<Mutex<HashMap<String, Node>>>,
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
}

#[derive(Debug, Clone)]
pub struct MySqlNodeRepository {
    pool: Pool,
    table_name: String,
}

impl MySqlNodeRepository {
    pub async fn new(config: MysqlConfig, table_prefix: String) -> Result<Self> {
        let opts = mysql_async::Opts::from_url(&config.connection_url())
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;
        let repository = Self {
            pool: Pool::new(opts),
            table_name: format!("{}nodes", table_prefix),
        };
        repository.init_table().await?;
        Ok(repository)
    }

    async fn init_table(&self) -> Result<()> {
        let mut conn = self.connection().await?;
        let create_table_sql = format!(
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
        conn.query_drop(create_table_sql)
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

    pub async fn reset_for_tests(&self) -> Result<()> {
        let mut conn = self.connection().await?;
        let drop_table_sql = format!("DROP TABLE IF EXISTS `{}`", self.table_name);
        conn.query_drop(drop_table_sql)
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
}
