use serde::{Deserialize, Serialize};

pub const AUTO_PROVISION_AGENT_ID: &str = "__AUTO_PROVISION__";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeConfig {
    pub nodemanage_sync_url: String,

    pub agent_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
}

fn default_data_dir() -> String {
    "/var/lib/rsagent".to_string()
}

fn default_sync_interval_secs() -> u64 {
    60
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            nodemanage_sync_url: String::new(),
            agent_id: String::new(),
            node_id: None,
            data_dir: default_data_dir(),
            sync_interval_secs: default_sync_interval_secs(),
        }
    }
}

impl AgentRuntimeConfig {
    pub fn installer_bootstrap(nodemanage_sync_url: String, install_root: String) -> Self {
        Self {
            nodemanage_sync_url,
            agent_id: AUTO_PROVISION_AGENT_ID.to_string(),
            node_id: None,
            data_dir: install_root,
            sync_interval_secs: default_sync_interval_secs(),
        }
    }
}
