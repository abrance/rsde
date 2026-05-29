use serde::{Deserialize, Serialize};

use crate::mysql::MysqlConfig;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatDataLinkConfig {
    #[serde(default = "default_heartbeat_result_table_name")]
    pub result_table_name: String,

    #[serde(default = "default_heartbeat_metric_name")]
    pub metric_name: String,

    #[serde(default = "default_heartbeat_query_template")]
    pub query_template: String,

    #[serde(default = "default_heartbeat_storage_cluster")]
    pub storage_cluster: String,

    #[serde(default = "default_heartbeat_interval_seconds")]
    pub interval_seconds: u64,

    #[serde(default = "default_heartbeat_retention_days")]
    pub retention_days: u32,

    #[serde(default = "default_refresh_interval_secs")]
    pub refresh_interval_secs: u64,

    #[serde(default = "default_status_window_secs")]
    pub status_window_secs: u64,
}

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

    #[serde(default)]
    pub heartbeat: HeartbeatDataLinkConfig,
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

fn default_heartbeat_result_table_name() -> String {
    "nm_node_heartbeat".to_string()
}

fn default_heartbeat_metric_name() -> String {
    "nm_node_heartbeat".to_string()
}

fn default_heartbeat_query_template() -> String {
    "query $metric_name where node_id = $node_id and agent_id = $agent_id and node_ip = $node_ip between $start_at and $end_at".to_string()
}

fn default_heartbeat_storage_cluster() -> String {
    "default".to_string()
}

fn default_heartbeat_interval_seconds() -> u64 {
    60
}

fn default_heartbeat_retention_days() -> u32 {
    7
}

fn default_refresh_interval_secs() -> u64 {
    60
}

fn default_status_window_secs() -> u64 {
    300
}

impl Default for HeartbeatDataLinkConfig {
    fn default() -> Self {
        Self {
            result_table_name: default_heartbeat_result_table_name(),
            metric_name: default_heartbeat_metric_name(),
            query_template: default_heartbeat_query_template(),
            storage_cluster: default_heartbeat_storage_cluster(),
            interval_seconds: default_heartbeat_interval_seconds(),
            retention_days: default_heartbeat_retention_days(),
            refresh_interval_secs: default_refresh_interval_secs(),
            status_window_secs: default_status_window_secs(),
        }
    }
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
            heartbeat: HeartbeatDataLinkConfig::default(),
        }
    }
}
