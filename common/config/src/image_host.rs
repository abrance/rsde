use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageHostingConfig {
    /// image file storage directory
    pub storage_dir: String,

    /// 定时清理间隔时间, 单位秒, 默认 3600 秒 (1 小时)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,

    /// 文件过期时间, 单位秒, 默认 3600 秒 (1 小时)
    #[serde(default = "default_file_expire")]
    pub file_expire_secs: u64,
}

fn default_cleanup_interval() -> u64 {
    3600 // 1 小时
}

fn default_file_expire() -> u64 {
    3600 // 1 小时
}

impl Default for ImageHostingConfig {
    fn default() -> Self {
        Self {
            storage_dir: String::new(),
            cleanup_interval_secs: default_cleanup_interval(),
            file_expire_secs: default_file_expire(),
        }
    }
}
