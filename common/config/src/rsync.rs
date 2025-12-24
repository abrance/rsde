//! Rsync 服务配置

use serde::{Deserialize, Serialize};

/// Rsync 全局配置
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct RsyncConfig {
    /// 元数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,

    /// 全局设置
    #[serde(default)]
    pub global: GlobalSettings,

    /// API 配置
    #[serde(default)]
    pub api: ApiConfig,

    /// 日志配置
    #[serde(default)]
    pub log: LogConfig,
}

/// 元数据
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    /// 配置 ID
    pub id: String,
    /// 配置名称
    pub name: String,
    /// 配置描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 全局设置
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct GlobalSettings {
    /// 是否启用调试模式
    #[serde(default)]
    pub debug: bool,
}

/// API 服务配置
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiConfig {
    /// 监听地址
    #[serde(default = "default_listen_address")]
    pub listen_address: String,

    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// 是否启用指标
    #[serde(default)]
    pub metrics_enabled: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            listen_address: default_listen_address(),
            log_level: default_log_level(),
            metrics_enabled: false,
        }
    }
}

fn default_listen_address() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

/// 日志配置
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogConfig {
    /// 日志路径
    #[serde(default = "default_log_path")]
    pub path: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            path: default_log_path(),
        }
    }
}

fn default_log_path() -> String {
    "./log/".to_string()
}

impl RsyncConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
