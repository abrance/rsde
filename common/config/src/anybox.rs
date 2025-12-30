use serde::{Deserialize, Serialize};

/// Anybox 配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnyboxConfig {
    /// Redis 连接 URL
    pub redis_url: String,

    /// 键前缀
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,

    /// 清理过期内容的间隔时间（秒）
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,
}

fn default_key_prefix() -> String {
    "anybox".to_string()
}

fn default_cleanup_interval() -> u64 {
    3600 // 1 小时
}

impl Default for AnyboxConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            key_prefix: default_key_prefix(),
            cleanup_interval_secs: default_cleanup_interval(),
        }
    }
}
