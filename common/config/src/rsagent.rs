use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const AUTO_PROVISION_AGENT_ID: &str = "__AUTO_PROVISION__";
const LOCAL_IDENTITY_FILE_NAME: &str = "agent-identity.toml";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct PersistedAgentIdentity {
    agent_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
}

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

    pub fn resolve_local_identity(&self) -> anyhow::Result<Self> {
        let persisted = self.load_persisted_identity()?;
        let agent_id = if self.agent_id == AUTO_PROVISION_AGENT_ID {
            persisted
                .as_ref()
                .map(|identity| identity.agent_id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            self.agent_id.clone()
        };
        let node_id = persisted
            .and_then(|identity| identity.node_id)
            .or_else(|| self.node_id.clone());

        Ok(Self {
            nodemanage_sync_url: self.nodemanage_sync_url.clone(),
            agent_id,
            node_id,
            data_dir: self.data_dir.clone(),
            sync_interval_secs: self.sync_interval_secs,
        })
    }

    pub fn persist_local_identity(&self) -> anyhow::Result<()> {
        self.persist_identity(self.agent_id.clone(), self.node_id.clone())
    }

    pub fn persist_identity(
        &self,
        agent_id: String,
        node_id: Option<String>,
    ) -> anyhow::Result<()> {
        fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("failed to create rsagent data dir {}", self.data_dir))?;
        let payload = toml::to_string(&PersistedAgentIdentity { agent_id, node_id })
            .context("failed to encode persisted rsagent identity")?;
        atomic_write(&self.local_identity_path(), &payload).with_context(|| {
            format!(
                "failed to persist rsagent identity under {}",
                self.local_identity_path().display()
            )
        })
    }

    fn load_persisted_identity(&self) -> anyhow::Result<Option<PersistedAgentIdentity>> {
        let path = self.local_identity_path();
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&path).with_context(|| {
            format!(
                "failed to read persisted rsagent identity from {}",
                path.display()
            )
        })?;
        let persisted = toml::from_str(&raw).with_context(|| {
            format!(
                "failed to decode persisted rsagent identity from {}",
                path.display()
            )
        })?;
        Ok(Some(persisted))
    }

    fn local_identity_path(&self) -> PathBuf {
        PathBuf::from(&self.data_dir).join(LOCAL_IDENTITY_FILE_NAME)
    }
}

fn atomic_write(path: &Path, payload: &str) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent.join(format!(
        "{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("agent-identity.toml"),
        std::process::id(),
        Uuid::new_v4()
    ));

    let mut file = fs::File::create(&temp_path).with_context(|| {
        format!(
            "failed to create temp identity file {}",
            temp_path.display()
        )
    })?;
    file.write_all(payload.as_bytes())
        .with_context(|| format!("failed to write temp identity file {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush temp identity file {}", temp_path.display()))?;
    drop(file);

    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "failed to atomically replace persisted identity {} with {}",
            path.display(),
            temp_path.display()
        )
    })?;

    let dir = fs::File::open(parent)
        .with_context(|| format!("failed to open identity dir {} for sync", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync identity dir {}", parent.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{AUTO_PROVISION_AGENT_ID, AgentRuntimeConfig, atomic_write};

    #[test]
    fn atomic_write_overwrites_existing_file_without_leaving_temp_siblings() {
        let data_dir = test_data_dir("atomic-overwrite");
        let identity_path = Path::new(&data_dir).join("agent-identity.toml");

        fs::write(&identity_path, "agent_id = \"agt-initial\"\n")
            .expect("seed initial identity file");
        atomic_write(&identity_path, "agent_id = \"agt-updated\"\n").expect("atomic overwrite");

        assert_eq!(
            fs::read_to_string(&identity_path).expect("read updated identity"),
            "agent_id = \"agt-updated\"\n"
        );
        assert!(!identity_temp_files_exist(&data_dir));
    }

    #[test]
    fn persist_local_identity_keeps_public_behavior_for_overwrite_and_restart() {
        let config = sample_config(test_data_dir("persist-restart"));

        config
            .persist_identity("agt-initial".to_string(), Some("node-initial".to_string()))
            .expect("persist initial identity");
        config
            .persist_identity("agt-stable".to_string(), Some("node-stable".to_string()))
            .expect("persist stable identity");

        let resolved = config
            .resolve_local_identity()
            .expect("resolve stable identity");
        assert_eq!(resolved.agent_id, "agt-stable");
        assert_eq!(resolved.node_id.as_deref(), Some("node-stable"));
        assert_ne!(resolved.agent_id, AUTO_PROVISION_AGENT_ID);
        assert!(!identity_temp_files_exist(&config.data_dir));
    }

    fn sample_config(data_dir: String) -> AgentRuntimeConfig {
        AgentRuntimeConfig {
            nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
            agent_id: AUTO_PROVISION_AGENT_ID.to_string(),
            node_id: None,
            data_dir,
            sync_interval_secs: 60,
        }
    }

    fn test_data_dir(name: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "config-rsagent-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create test data dir");
        path.to_string_lossy().into_owned()
    }

    fn identity_temp_files_exist(data_dir: &str) -> bool {
        fs::read_dir(data_dir)
            .expect("read data dir")
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("agent-identity.toml.tmp")
            })
    }
}
