//! MySQL 客户端封装
//!
//! 提供 MySQL 连接和基本操作的封装，支持：
//! - 异步连接
//! - 密码认证
//! - SSL/TLS 支持
//! - 连接池管理
//! - 基本的 CRUD 操作
//!
//! # 示例
//!
//! ```ignore
//! use util::client::mysql::{MySqlClientConfig, MySqlClient};
//!
//! let config = MySqlClientConfig::new("localhost:3306")
//!     .with_username("root")
//!     .with_password("secret")
//!     .with_database("test");
//!
//! let client = MySqlClient::new(&config)?;
//! client.ping().await?;
//! client.execute_ddl("CREATE TABLE IF NOT EXISTS test (id INT, name VARCHAR(255))").await?;
//! let rows_affected = client.execute_dml("INSERT INTO test VALUES (1, 'test')").await?;
//! ```

use mysql_async::{Opts, OptsBuilder, Params, Pool, prelude::*};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// MySQL 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlClientConfig {
    /// MySQL 服务器地址（格式：host:port 或 host）
    pub host: String,
    /// 端口（默认：3306）
    pub port: u16,
    /// 用户名
    pub username: Option<String>,
    /// 密码（可选）
    pub password: Option<String>,
    /// 数据库名称（可选）
    pub database: Option<String>,
    /// 连接超时时间（秒）
    pub timeout: Option<u64>,
    /// 是否启用 SSL/TLS
    pub ssl: bool,
}

impl MySqlClientConfig {
    /// 创建一个新的 MySQL 客户端配置
    ///
    /// # 参数
    /// - `host`: MySQL 服务器地址，支持以下格式：
    ///   - `host:port` - 主机和端口
    ///   - `host` - 自动使用默认端口 3306
    pub fn new(host: impl Into<String>) -> Self {
        let host_str = host.into();
        let (host, port) = if let Some(pos) = host_str.find(':') {
            let host_part = &host_str[..pos];
            let port_part = &host_str[pos + 1..];
            let port = port_part.parse().unwrap_or(3306);
            (host_part.to_string(), port)
        } else {
            (host_str, 3306)
        };

        Self {
            host,
            port,
            username: None,
            password: None,
            database: None,
            timeout: Some(10),
            ssl: false,
        }
    }

    /// 设置用户名
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// 设置密码
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// 设置数据库名称
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// 设置连接超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 启用 SSL/TLS
    pub fn with_ssl(mut self, ssl: bool) -> Self {
        self.ssl = ssl;
        self
    }

    /// 构建 MySQL 连接选项
    fn build_opts(&self) -> Opts {
        let mut builder = OptsBuilder::default()
            .ip_or_hostname(self.host.clone())
            .tcp_port(self.port)
            .prefer_socket(false);

        if let Some(username) = &self.username {
            builder = builder.user(Some(username));
        }

        if let Some(password) = &self.password {
            builder = builder.pass(Some(password));
        }

        if let Some(database) = &self.database {
            builder = builder.db_name(Some(database));
        }

        if self.ssl {
            builder = builder.ssl_opts(Some(mysql_async::SslOpts::default()));
        }

        if let Some(timeout) = self.timeout {
            builder = builder.conn_ttl(Some(Duration::from_secs(timeout)));
        }

        builder.into()
    }
}

/// MySQL 客户端
///
/// 使用连接池提供高效的异步连接
pub struct MySqlClient {
    pool: Pool,
    config: MySqlClientConfig,
}

impl MySqlClient {
    /// 创建一个新的 MySQL 客户端
    pub async fn new(config: &MySqlClientConfig) -> Result<Self, String> {
        let opts = config.build_opts();

        // Create pool directly with options
        let pool = Pool::new(opts);

        // Test connection
        let mut conn = pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to connect to MySQL: {}", e))?;

        // Ping to verify connection
        conn.ping()
            .await
            .map_err(|e| format!("Failed to ping MySQL: {}", e))?;

        Ok(Self {
            pool,
            config: config.clone(),
        })
    }

    /// 检查与 MySQL 的连接是否正常（PING）
    pub async fn ping(&mut self) -> Result<(), String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;
        conn.ping().await.map_err(|e| format!("Ping failed: {}", e))
    }

    /// 获取 MySQL 服务器信息
    pub async fn info(&mut self) -> Result<String, String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        let result: Vec<(String, String)> =
            conn.exec("SHOW VARIABLES LIKE 'version%'", ())
                .await
                .map_err(|e| format!("Failed to get server info: {}", e))?;

        let mut info = String::new();
        for (name, value) in result {
            info.push_str(&format!("{}: {}\n", name, value));
        }
        Ok(info)
    }

    /// 获取 MySQL 服务器版本
    pub async fn version(&mut self) -> Result<String, String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        let version: String = conn
            .exec_first("SELECT VERSION()", ())
            .await
            .map_err(|e| format!("Failed to get version: {}", e))?
            .ok_or_else(|| "Could not determine MySQL version".to_string())?;

        Ok(version)
    }

    /// 执行 DDL SQL 语句（不返回结果）
    ///
    /// 适用于 CREATE, DROP, ALTER 等数据定义语言
    pub async fn execute_ddl(&mut self, query: &str) -> Result<(), String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        conn.exec_drop(query, ())
            .await
            .map_err(|e| format!("Failed to execute DDL query: {}", e))
    }

    /// 执行 DML SQL 语句（返回受影响的行数）
    ///
    /// 适用于 INSERT, UPDATE, DELETE 等数据操作语言
    pub async fn execute_dml(&mut self, query: &str) -> Result<u64, String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        let result = conn
            .exec_iter(query, ())
            .await
            .map_err(|e| format!("Failed to execute DML query: {}", e))?;

        Ok(result.affected_rows())
    }

    /// 执行查询并返回结果
    pub async fn query<T>(&mut self, query: &str) -> Result<Vec<T>, String>
    where
        T: FromRow + Send + 'static,
    {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        let result = conn
            .exec(query, ())
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        Ok(result)
    }

    /// 执行参数化查询
    pub async fn query_with_params<T, P>(
        &mut self,
        query: &str,
        params: P,
    ) -> Result<Vec<T>, String>
    where
        T: FromRow + Send + 'static,
        P: Into<Params> + Send,
    {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;

        let result = conn
            .exec(query, params)
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        Ok(result)
    }

    /// 插入数据并返回插入的行数
    pub async fn insert(&mut self, query: &str) -> Result<u64, String> {
        self.execute_dml(query).await
    }

    /// 更新数据并返回受影响的行数
    pub async fn update(&mut self, query: &str) -> Result<u64, String> {
        self.execute_dml(query).await
    }

    /// 删除数据并返回受影响的行数
    pub async fn delete(&mut self, query: &str) -> Result<u64, String> {
        self.execute_dml(query).await
    }

    /// 验证连接池中的连接是否有效
    pub async fn validate_connection(&self) -> Result<(), String> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| format!("Failed to get connection: {}", e))?;
        conn.ping()
            .await
            .map_err(|e| format!("Connection validation failed: {}", e))
    }

    /// 获取配置信息
    pub fn get_config(&self) -> &MySqlClientConfig {
        &self.config
    }

    /// 获取原始连接池（用于高级用法）
    pub fn get_pool(&self) -> &Pool {
        &self.pool
    }
}

/// MySQL 连接测试结果
#[derive(Debug, Serialize, Deserialize)]
pub struct MySqlPingResult {
    pub success: bool,
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = MySqlClientConfig::new("localhost:3306")
            .with_username("root")
            .with_password("secret")
            .with_database("test")
            .with_timeout(30);

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3306);
        assert_eq!(config.username, Some("root".to_string()));
        assert_eq!(config.password, Some("secret".to_string()));
        assert_eq!(config.database, Some("test".to_string()));
        assert_eq!(config.timeout, Some(30));
        assert!(!config.ssl);
    }

    #[test]
    fn test_config_with_default_port() {
        let config = MySqlClientConfig::new("localhost");
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3306);
    }

    #[test]
    fn test_config_with_ssl() {
        let config = MySqlClientConfig::new("localhost:3306").with_ssl(true);
        assert!(config.ssl);
    }

    #[test]
    fn test_build_opts() {
        let config = MySqlClientConfig::new("localhost:3306")
            .with_username("user")
            .with_password("pass")
            .with_database("test_db");

        let opts = config.build_opts();
        // We can't easily test the internal opts, but this ensures it doesn't panic
        assert_eq!(opts.ip_or_hostname(), "localhost");
    }
}
