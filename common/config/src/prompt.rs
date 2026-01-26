use serde::{Deserialize, Serialize};

use crate::mysql::MysqlConfig;

/// Prompt 服务配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PromptConfig {
    /// MySQL 配置
    pub mysql: MysqlConfig,

    /// 表名前缀
    #[serde(default = "default_table_prefix")]
    pub table_prefix: String,
}

fn default_table_prefix() -> String {
    "prompt_".to_string()
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            mysql: MysqlConfig::default(),
            table_prefix: default_table_prefix(),
        }
    }
}
