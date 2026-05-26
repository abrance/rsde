use std::time::Duration;

use config::mysql::MysqlConfig;
use sea_orm::{DatabaseConnection, SqlxMySqlConnector};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};

/// MySQL database client configuration shared by sqlx and SeaORM.
///
/// # Example
///
/// ```ignore
/// use config::mysql::MysqlConfig;
/// use util::db::mysql::{DatabaseClientConfig, MySqlDatabaseClient};
///
/// let mysql_config = MysqlConfig::default();
/// let db_config = DatabaseClientConfig::from(mysql_config);
/// let client = MySqlDatabaseClient::connect(&db_config).await?;
///
/// // SeaORM entity CRUD.
/// let db = client.sea_orm();
///
/// // Raw SQL or bulk operations.
/// let pool = client.sqlx_pool();
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseClientConfig {
    database_url: String,
    max_connections: u32,
    min_connections: u32,
    connect_timeout_secs: u64,
}

impl DatabaseClientConfig {
    /// Creates a database client configuration from an explicit MySQL URL.
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_secs: 10,
        }
    }

    /// Overrides the maximum number of connections in the pool.
    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Overrides the minimum number of connections in the pool.
    pub fn with_min_connections(mut self, min_connections: u32) -> Self {
        self.min_connections = min_connections;
        self
    }

    /// Overrides the connect timeout in seconds.
    pub fn with_connect_timeout_secs(mut self, connect_timeout_secs: u64) -> Self {
        self.connect_timeout_secs = connect_timeout_secs;
        self
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    pub fn min_connections(&self) -> u32 {
        self.min_connections
    }

    pub fn connect_timeout_secs(&self) -> u64 {
        self.connect_timeout_secs
    }
}

impl From<MysqlConfig> for DatabaseClientConfig {
    fn from(config: MysqlConfig) -> Self {
        Self {
            database_url: config.connection_url(),
            max_connections: config.max_connections,
            min_connections: config.min_connections,
            connect_timeout_secs: config.connect_timeout_secs,
        }
    }
}

/// MySQL database client exposing both sqlx and SeaORM handles over one pool.
#[derive(Clone)]
pub struct MySqlDatabaseClient {
    sqlx_pool: MySqlPool,
    sea_orm: DatabaseConnection,
}

impl MySqlDatabaseClient {
    /// Connects to MySQL and creates shared sqlx/SeaORM handles.
    pub async fn connect(config: &DatabaseClientConfig) -> Result<Self, sqlx::Error> {
        let sqlx_pool = MySqlPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
            .connect(config.database_url())
            .await?;
        let sea_orm = SqlxMySqlConnector::from_sqlx_mysql_pool(sqlx_pool.clone());

        Ok(Self { sqlx_pool, sea_orm })
    }

    /// Returns the raw sqlx pool for custom SQL or bulk operations.
    pub fn sqlx_pool(&self) -> &MySqlPool {
        &self.sqlx_pool
    }

    /// Returns the SeaORM connection for entity-based CRUD operations.
    pub fn sea_orm(&self) -> &DatabaseConnection {
        &self.sea_orm
    }

    /// Closes the shared sqlx pool. SeaORM clones backed by the same pool stop accepting new work.
    pub async fn close(&self) {
        self.sqlx_pool.close().await;
    }
}
