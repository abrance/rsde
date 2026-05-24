use serde::{Deserialize, Serialize};

use crate::mysql::MysqlConfig;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeManageConfig {
    #[serde(default = "default_table_prefix")]
    pub table_prefix: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mysql: Option<MysqlConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsagent_package_url: Option<String>,

    #[serde(default = "default_ssh_connect_timeout_secs")]
    pub ssh_connect_timeout_secs: u64,
}

fn default_table_prefix() -> String {
    "node_".to_string()
}

fn default_ssh_connect_timeout_secs() -> u64 {
    10
}

impl Default for NodeManageConfig {
    fn default() -> Self {
        Self {
            table_prefix: default_table_prefix(),
            mysql: None,
            rsagent_package_url: None,
            ssh_connect_timeout_secs: default_ssh_connect_timeout_secs(),
        }
    }
}
