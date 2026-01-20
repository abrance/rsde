//! Redis 客户端封装
//!
//! 提供 Redis 连接和基本操作的封装，支持：
//! - 单节点连接
//! - 密码认证
//! - TLS 支持
//! - 连接池管理
//!
//! # 示例
//!
//! ```ignore
//! use util::client::redis::{RedisClientConfig, RedisClient};
//!
//! let config = RedisClientConfig::new("redis://localhost:6379")
//!     .with_password("secret")
//!     .with_db(0);
//!
//! let client = RedisClient::new(&config)?;
//! client.ping().await?;
//! client.set("key", "value").await?;
//! let value: String = client.get("key").await?;
//! ```

use redis::{AsyncCommands, Client, aio::ConnectionManager};
use serde::{Deserialize, Serialize};

/// Redis 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisClientConfig {
    /// Redis 服务器地址（格式：redis://host:port 或 rediss://host:port）
    pub url: String,
    /// 密码（可选）
    pub password: Option<String>,
    /// 数据库索引（默认：0）
    pub db: Option<i64>,
    /// 用户名（Redis 6.0+ ACL 支持）
    pub username: Option<String>,
    /// 连接超时时间（秒）
    pub timeout: Option<u64>,
    /// 是否启用 TLS
    pub tls: bool,
}

impl RedisClientConfig {
    /// 创建一个新的 Redis 客户端配置
    ///
    /// # 参数
    /// - `url`: Redis 服务器地址，支持以下格式：
    ///   - `redis://host:port` - 普通连接
    ///   - `rediss://host:port` - TLS 连接
    ///   - `host:port` - 自动添加 redis:// 前缀
    pub fn new(url: impl Into<String>) -> Self {
        let url_str = url.into();
        let (url, tls) = if url_str.starts_with("rediss://") {
            (url_str, true)
        } else if url_str.starts_with("redis://") {
            (url_str, false)
        } else {
            (format!("redis://{}", url_str), false)
        };

        Self {
            url,
            password: None,
            db: None,
            username: None,
            timeout: Some(10),
            tls,
        }
    }

    /// 设置密码
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// 设置数据库索引
    pub fn with_db(mut self, db: i64) -> Self {
        self.db = Some(db);
        self
    }

    /// 设置用户名（Redis 6.0+ ACL）
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// 设置连接超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 启用 TLS
    pub fn with_tls(mut self, tls: bool) -> Self {
        self.tls = tls;
        if tls && self.url.starts_with("redis://") {
            self.url = self.url.replace("redis://", "rediss://");
        }
        self
    }

    /// 构建 Redis 连接 URL
    fn build_connection_url(&self) -> String {
        let mut url = self.url.clone();

        // 解析并重构 URL 以添加认证信息
        if self.username.is_some() || self.password.is_some() || self.db.is_some() {
            let scheme = if self.tls { "rediss://" } else { "redis://" };
            let host_port = url
                .trim_start_matches("redis://")
                .trim_start_matches("rediss://");

            let auth = match (&self.username, &self.password) {
                (Some(user), Some(pass)) => format!("{}:{}@", user, pass),
                (None, Some(pass)) => format!(":{}@", pass),
                _ => String::new(),
            };

            let db_suffix = self.db.map(|db| format!("/{}", db)).unwrap_or_default();

            url = format!("{}{}{}{}", scheme, auth, host_port, db_suffix);
        }

        url
    }
}

/// Redis 客户端
///
/// 使用 ConnectionManager 提供自动重连的异步连接
pub struct RedisClient {
    connection: ConnectionManager,
    config: RedisClientConfig,
}

impl RedisClient {
    /// 创建一个新的 Redis 客户端
    pub async fn new(config: &RedisClientConfig) -> Result<Self, String> {
        let url = config.build_connection_url();

        let client =
            Client::open(url).map_err(|e| format!("Failed to create Redis client: {}", e))?;

        let connection = ConnectionManager::new(client)
            .await
            .map_err(|e| format!("Failed to connect to Redis: {}", e))?;

        Ok(Self {
            connection,
            config: config.clone(),
        })
    }

    /// 检查与 Redis 的连接是否正常（PING）
    pub async fn ping(&mut self) -> Result<String, String> {
        redis::cmd("PING")
            .query_async(&mut self.connection)
            .await
            .map_err(|e| format!("Ping failed: {}", e))
    }

    /// 获取 Redis 服务器信息
    pub async fn info(&mut self, section: Option<&str>) -> Result<String, String> {
        let mut cmd = redis::cmd("INFO");
        if let Some(s) = section {
            cmd.arg(s);
        }
        cmd.query_async(&mut self.connection)
            .await
            .map_err(|e| format!("Failed to get INFO: {}", e))
    }

    /// 获取 Redis 服务器版本
    pub async fn version(&mut self) -> Result<String, String> {
        let info: String = self.info(Some("server")).await?;
        for line in info.lines() {
            if line.starts_with("redis_version:") {
                return Ok(line.trim_start_matches("redis_version:").to_string());
            }
        }
        Err("Could not determine Redis version".to_string())
    }

    /// 设置键值对
    pub async fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.connection
            .set(key, value)
            .await
            .map_err(|e| format!("Failed to SET: {}", e))
    }

    /// 设置键值对（带过期时间）
    pub async fn set_ex(&mut self, key: &str, value: &str, seconds: u64) -> Result<(), String> {
        self.connection
            .set_ex(key, value, seconds)
            .await
            .map_err(|e| format!("Failed to SETEX: {}", e))
    }

    /// 获取键值
    pub async fn get(&mut self, key: &str) -> Result<Option<String>, String> {
        self.connection
            .get(key)
            .await
            .map_err(|e| format!("Failed to GET: {}", e))
    }

    /// 删除键
    pub async fn del(&mut self, key: &str) -> Result<i64, String> {
        self.connection
            .del(key)
            .await
            .map_err(|e| format!("Failed to DEL: {}", e))
    }

    /// 检查键是否存在
    pub async fn exists(&mut self, key: &str) -> Result<bool, String> {
        self.connection
            .exists(key)
            .await
            .map_err(|e| format!("Failed to EXISTS: {}", e))
    }

    /// 设置键的过期时间（秒）
    pub async fn expire(&mut self, key: &str, seconds: i64) -> Result<bool, String> {
        self.connection
            .expire(key, seconds)
            .await
            .map_err(|e| format!("Failed to EXPIRE: {}", e))
    }

    /// 获取键的剩余过期时间（秒）
    pub async fn ttl(&mut self, key: &str) -> Result<i64, String> {
        self.connection
            .ttl(key)
            .await
            .map_err(|e| format!("Failed to TTL: {}", e))
    }

    /// 获取匹配模式的键列表
    pub async fn keys(&mut self, pattern: &str) -> Result<Vec<String>, String> {
        self.connection
            .keys(pattern)
            .await
            .map_err(|e| format!("Failed to KEYS: {}", e))
    }

    /// 获取数据库中键的数量
    pub async fn dbsize(&mut self) -> Result<i64, String> {
        redis::cmd("DBSIZE")
            .query_async(&mut self.connection)
            .await
            .map_err(|e| format!("Failed to DBSIZE: {}", e))
    }

    /// 清空当前数据库
    pub async fn flushdb(&mut self) -> Result<(), String> {
        redis::cmd("FLUSHDB")
            .query_async(&mut self.connection)
            .await
            .map_err(|e| format!("Failed to FLUSHDB: {}", e))
    }

    /// 获取配置信息
    pub fn get_config(&self) -> &RedisClientConfig {
        &self.config
    }

    /// 执行原始 Redis 命令
    pub async fn execute<T: redis::FromRedisValue>(
        &mut self,
        cmd: &str,
        args: &[&str],
    ) -> Result<T, String> {
        let mut command = redis::cmd(cmd);
        for arg in args {
            command.arg(*arg);
        }
        command
            .query_async(&mut self.connection)
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))
    }

    // ========== 列表操作 ==========

    /// 从左侧推入列表
    pub async fn lpush(&mut self, key: &str, value: &str) -> Result<i64, String> {
        self.connection
            .lpush(key, value)
            .await
            .map_err(|e| format!("Failed to LPUSH: {}", e))
    }

    /// 从右侧推入列表
    pub async fn rpush(&mut self, key: &str, value: &str) -> Result<i64, String> {
        self.connection
            .rpush(key, value)
            .await
            .map_err(|e| format!("Failed to RPUSH: {}", e))
    }

    /// 从左侧弹出列表元素
    pub async fn lpop(&mut self, key: &str) -> Result<Option<String>, String> {
        self.connection
            .lpop(key, None)
            .await
            .map_err(|e| format!("Failed to LPOP: {}", e))
    }

    /// 从右侧弹出列表元素
    pub async fn rpop(&mut self, key: &str) -> Result<Option<String>, String> {
        self.connection
            .rpop(key, None)
            .await
            .map_err(|e| format!("Failed to RPOP: {}", e))
    }

    /// 获取列表范围
    pub async fn lrange(
        &mut self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<String>, String> {
        self.connection
            .lrange(key, start, stop)
            .await
            .map_err(|e| format!("Failed to LRANGE: {}", e))
    }

    /// 获取列表长度
    pub async fn llen(&mut self, key: &str) -> Result<i64, String> {
        self.connection
            .llen(key)
            .await
            .map_err(|e| format!("Failed to LLEN: {}", e))
    }

    // ========== 哈希表操作 ==========

    /// 设置哈希表字段
    pub async fn hset(&mut self, key: &str, field: &str, value: &str) -> Result<bool, String> {
        self.connection
            .hset(key, field, value)
            .await
            .map_err(|e| format!("Failed to HSET: {}", e))
    }

    /// 获取哈希表字段
    pub async fn hget(&mut self, key: &str, field: &str) -> Result<Option<String>, String> {
        self.connection
            .hget(key, field)
            .await
            .map_err(|e| format!("Failed to HGET: {}", e))
    }

    /// 获取哈希表所有字段和值
    pub async fn hgetall(&mut self, key: &str) -> Result<Vec<(String, String)>, String> {
        self.connection
            .hgetall(key)
            .await
            .map_err(|e| format!("Failed to HGETALL: {}", e))
    }

    /// 删除哈希表字段
    pub async fn hdel(&mut self, key: &str, field: &str) -> Result<i64, String> {
        self.connection
            .hdel(key, field)
            .await
            .map_err(|e| format!("Failed to HDEL: {}", e))
    }

    // ========== 集合操作 ==========

    /// 添加集合成员
    pub async fn sadd(&mut self, key: &str, member: &str) -> Result<i64, String> {
        self.connection
            .sadd(key, member)
            .await
            .map_err(|e| format!("Failed to SADD: {}", e))
    }

    /// 获取集合所有成员
    pub async fn smembers(&mut self, key: &str) -> Result<Vec<String>, String> {
        self.connection
            .smembers(key)
            .await
            .map_err(|e| format!("Failed to SMEMBERS: {}", e))
    }

    /// 检查是否为集合成员
    pub async fn sismember(&mut self, key: &str, member: &str) -> Result<bool, String> {
        self.connection
            .sismember(key, member)
            .await
            .map_err(|e| format!("Failed to SISMEMBER: {}", e))
    }

    /// 获取集合大小
    pub async fn scard(&mut self, key: &str) -> Result<i64, String> {
        self.connection
            .scard(key)
            .await
            .map_err(|e| format!("Failed to SCARD: {}", e))
    }
}

/// Redis 连接测试结果
#[derive(Debug, Serialize, Deserialize)]
pub struct RedisPingResult {
    pub success: bool,
    pub url: String,
    pub db: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dbsize: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = RedisClientConfig::new("localhost:6379")
            .with_password("secret")
            .with_db(1)
            .with_timeout(30);

        assert!(config.url.starts_with("redis://"));
        assert_eq!(config.password, Some("secret".to_string()));
        assert_eq!(config.db, Some(1));
        assert_eq!(config.timeout, Some(30));
        assert!(!config.tls);
    }

    #[test]
    fn test_config_with_redis_prefix() {
        let config = RedisClientConfig::new("redis://localhost:6379");
        assert_eq!(config.url, "redis://localhost:6379");
        assert!(!config.tls);
    }

    #[test]
    fn test_config_with_tls() {
        let config = RedisClientConfig::new("rediss://localhost:6379");
        assert_eq!(config.url, "rediss://localhost:6379");
        assert!(config.tls);
    }

    #[test]
    fn test_config_enable_tls() {
        let config = RedisClientConfig::new("localhost:6379").with_tls(true);
        assert!(config.url.starts_with("rediss://"));
        assert!(config.tls);
    }

    #[test]
    fn test_build_connection_url() {
        let config = RedisClientConfig::new("localhost:6379")
            .with_password("secret")
            .with_db(2);

        let url = config.build_connection_url();
        assert_eq!(url, "redis://:secret@localhost:6379/2");
    }

    #[test]
    fn test_build_connection_url_with_acl() {
        let config = RedisClientConfig::new("localhost:6379")
            .with_username("user")
            .with_password("pass")
            .with_db(0);

        let url = config.build_connection_url();
        assert_eq!(url, "redis://user:pass@localhost:6379/0");
    }

    #[test]
    fn test_build_connection_url_no_auth() {
        let config = RedisClientConfig::new("redis://localhost:6379");
        let url = config.build_connection_url();
        assert_eq!(url, "redis://localhost:6379");
    }
}
