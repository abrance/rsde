use serde::{Deserialize, Serialize};

/// MySQL 配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MysqlConfig {
    /// MySQL 主机地址
    #[serde(default = "default_host")]
    pub host: String,

    /// MySQL 端口
    #[serde(default = "default_port")]
    pub port: u16,

    /// 用户名
    #[serde(default = "default_user")]
    pub user: String,

    /// 密码
    #[serde(default)]
    pub password: String,

    /// 数据库名
    #[serde(default = "default_database")]
    pub database: String,

    /// 连接池最大连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// 连接池最小连接数
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// 连接超时时间（秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3306
}

fn default_user() -> String {
    "root".to_string()
}

fn default_database() -> String {
    "prompt".to_string()
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    1
}

fn default_connect_timeout() -> u64 {
    10
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            user: default_user(),
            password: String::new(),
            database: default_database(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connect_timeout_secs: default_connect_timeout(),
        }
    }
}

impl MysqlConfig {
    /// 生成 MySQL 连接 URL
    pub fn connection_url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        )
    }
}
