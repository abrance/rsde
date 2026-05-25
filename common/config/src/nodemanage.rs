use serde::{Deserialize, Serialize};

use crate::mysql::MysqlConfig;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InstallPluginConfig {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeManageConfig {
    #[serde(default = "default_table_prefix")]
    pub table_prefix: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mysql: Option<MysqlConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsagent_package_url: Option<String>,

    #[serde(default = "default_install_root")]
    pub install_root: String,

    #[serde(default = "default_register_callback_url")]
    pub register_callback_url: String,

    #[serde(default)]
    pub install_plugins: Vec<InstallPluginConfig>,

    #[serde(default = "default_register_wait_timeout_secs")]
    pub register_wait_timeout_secs: u64,

    #[serde(default = "default_ssh_connect_timeout_secs")]
    pub ssh_connect_timeout_secs: u64,
}

fn default_table_prefix() -> String {
    "node_".to_string()
}

fn default_ssh_connect_timeout_secs() -> u64 {
    10
}

fn default_install_root() -> String {
    "/opt/rsagent".to_string()
}

fn default_register_callback_url() -> String {
    "http://127.0.0.1:3000/api/nodes/agent/register".to_string()
}

fn default_register_wait_timeout_secs() -> u64 {
    30
}

impl Default for NodeManageConfig {
    fn default() -> Self {
        Self {
            table_prefix: default_table_prefix(),
            mysql: None,
            rsagent_package_url: None,
            install_root: default_install_root(),
            register_callback_url: default_register_callback_url(),
            install_plugins: vec![],
            register_wait_timeout_secs: default_register_wait_timeout_secs(),
            ssh_connect_timeout_secs: default_ssh_connect_timeout_secs(),
        }
    }
}
