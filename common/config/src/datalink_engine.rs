//! DataLink Engine 配置

use crate::mysql::MysqlConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DataLinkEngineBackend {
    #[default]
    Memory,
    Mysql,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DataLinkEngineConfig {
    #[serde(default)]
    pub backend: DataLinkEngineBackend,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mysql: Option<MysqlConfig>,
}
