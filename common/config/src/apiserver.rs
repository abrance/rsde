//! API Server 配置

use serde::{Deserialize, Serialize};

/// API Server 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiServerConfig {
    /// 监听地址
    #[serde(default = "default_listen_address")]
    pub listen_address: String,

    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// 是否启用 CORS
    #[serde(default = "default_cors_enabled")]
    pub cors_enabled: bool,
}

fn default_listen_address() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_cors_enabled() -> bool {
    true
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
            log_level: default_log_level(),
            cors_enabled: default_cors_enabled(),
        }
    }
}
