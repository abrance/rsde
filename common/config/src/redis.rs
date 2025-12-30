use serde::{Deserialize, Serialize};

fn default_redis_address() -> String {
    "127.0.0.1:6379".to_string()
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct RedisConfig {
    /// Redis 服务器地址
    #[serde(default = "default_redis_address")]
    pub address: String,

    /// 连接密码 (可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}
