use async_trait::async_trait;
use config::rsagent::AgentRuntimeConfig;
use serde::{Deserialize, Serialize};
use std::{process::Stdio, sync::Arc};
use tokio::{
    process::Command,
    time::{Duration, Instant, sleep},
};
use uuid::Uuid;

use crate::{NodeRepository, PaginationParams, Result};

fn default_install_root() -> String {
    "/opt/rsagent".to_string()
}

fn default_register_callback_url() -> String {
    "http://127.0.0.1:3000/agent/sync".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallPlugin {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuth {
    Password(String),
    PrivateKey(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshConnectionRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
}

impl SshConnectionRequest {
    pub fn from_parts(
        host: String,
        port: u16,
        username: String,
        password: Option<String>,
        private_key: Option<String>,
    ) -> Result<Self> {
        let auth = if let Some(password) = password {
            SshAuth::Password(password)
        } else if let Some(private_key) = private_key {
            SshAuth::PrivateKey(private_key)
        } else {
            return Err(crate::NodeManageError::InvalidInput(
                "ssh auth requires password or private_key".to_string(),
            ));
        };

        Ok(Self {
            host,
            port,
            username,
            auth,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRuntimeConfig {
    pub runtime: AgentRuntimeConfig,
}

impl InstallRuntimeConfig {
    pub fn new(install_root: String, register_callback_url: String) -> Self {
        let nodemanage_sync_url = normalize_sync_url(&register_callback_url);
        Self {
            runtime: AgentRuntimeConfig::installer_bootstrap(nodemanage_sync_url, install_root),
        }
    }

    pub fn render(&self) -> Result<String> {
        toml::to_string(&self.runtime)
            .map_err(|err| crate::NodeManageError::Storage(err.to_string()))
    }
}

fn normalize_sync_url(raw: &str) -> String {
    raw.trim_end_matches("/")
        .strip_suffix("/api/nodes/agent/register")
        .or_else(|| raw.trim_end_matches("/").strip_suffix("/agent/sync"))
        .unwrap_or(raw.trim_end_matches('/'))
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallStep {
    EnsureDirectory(String),
    DownloadRsAgent(String),
    DownloadPlugin { name: String, package_url: String },
    WriteRuntimeConfig,
    WriteInstallConf,
    StartAgent,
}

impl InstallStep {
    pub fn plan_for_fresh_install(
        install_root: String,
        rsagent_package_url: String,
        plugins: &[InstallPlugin],
    ) -> Vec<Self> {
        let mut steps = vec![
            Self::EnsureDirectory(install_root.clone()),
            Self::EnsureDirectory(format!("{install_root}/bin")),
            Self::EnsureDirectory(format!("{install_root}/config")),
            Self::EnsureDirectory(format!("{install_root}/plugin")),
            Self::DownloadRsAgent(rsagent_package_url),
        ];

        for plugin in plugins {
            if let Some(package_url) = &plugin.package_url {
                steps.push(Self::DownloadPlugin {
                    name: plugin.name.clone(),
                    package_url: package_url.clone(),
                });
            }
        }

        steps.push(Self::WriteRuntimeConfig);
        steps.push(Self::WriteInstallConf);
        steps.push(Self::StartAgent);
        steps
    }
}

#[async_trait]
pub trait RemoteExecutor: Send + Sync + 'static {
    async fn read_install_conf(
        &self,
        connection: &SshConnectionRequest,
        install_root: &str,
    ) -> Result<Option<String>>;

    async fn execute_install_plan(
        &self,
        connection: &SshConnectionRequest,
        steps: &[InstallStep],
        runtime_config: &str,
        install_conf: &str,
    ) -> Result<()>;
}

#[async_trait]
pub trait RegistrationWaiter: Send + Sync + 'static {
    async fn wait_for_registration(&self, host: &str, timeout_secs: u64) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct ShellRemoteExecutor;

impl ShellRemoteExecutor {
    fn build_ssh_command(connection: &SshConnectionRequest, script: &str) -> Command {
        let mut command = match &connection.auth {
            SshAuth::Password(password) => {
                let mut cmd = Command::new("sshpass");
                cmd.arg("-p").arg(password).arg("ssh");
                cmd
            }
            SshAuth::PrivateKey(private_key) => {
                let mut cmd = Command::new("ssh");
                cmd.arg("-i").arg(private_key);
                cmd
            }
        };

        command
            .arg("-p")
            .arg(connection.port.to_string())
            .arg(format!("{}@{}", connection.username, connection.host))
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn render_steps(
        &self,
        install_root: &str,
        steps: &[InstallStep],
        runtime_config: &str,
        install_conf: &str,
    ) -> String {
        let mut lines = vec!["set -e".to_string()];

        for step in steps {
            match step {
                InstallStep::EnsureDirectory(path) => {
                    lines.push(format!("mkdir -p '{path}'"));
                }
                InstallStep::DownloadRsAgent(url) => {
                    lines.push(format!(
                        "curl -fsSL '{url}' -o '{install_root}/bin/rsagent-package'"
                    ));
                }
                InstallStep::DownloadPlugin { name, package_url } => {
                    lines.push(format!(
                        "curl -fsSL '{package_url}' -o '{install_root}/plugin/{name}-plugin-package'"
                    ));
                }
                InstallStep::WriteRuntimeConfig => {
                    lines.push(format!(
                        "cat > '{install_root}/config/rsagent.toml' <<'EOF'\n{runtime_config}\nEOF"
                    ));
                }
                InstallStep::WriteInstallConf => {
                    lines.push(format!(
                        "cat > '{install_root}/install.conf' <<'EOF'\n{install_conf}\nEOF"
                    ));
                }
                InstallStep::StartAgent => {
                    lines.push(format!(
                        "if [ -x '{install_root}/bin/rsagent' ]; then '{install_root}/bin/rsagent' --config '{install_root}/config/rsagent.toml' >/tmp/rsagent.log 2>&1 & fi"
                    ));
                }
            }
        }

        lines.join("\n")
    }
}

#[async_trait]
impl RemoteExecutor for ShellRemoteExecutor {
    async fn read_install_conf(
        &self,
        connection: &SshConnectionRequest,
        install_root: &str,
    ) -> Result<Option<String>> {
        let script = format!(
            "if [ -f '{install_root}/install.conf' ]; then cat '{install_root}/install.conf'; fi"
        );
        let output = Self::build_ssh_command(connection, &script)
            .output()
            .await
            .map_err(|err| crate::NodeManageError::Storage(err.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(None)
        } else {
            Ok(Some(stdout))
        }
    }

    async fn execute_install_plan(
        &self,
        connection: &SshConnectionRequest,
        steps: &[InstallStep],
        runtime_config: &str,
        install_conf: &str,
    ) -> Result<()> {
        let install_root = steps
            .iter()
            .find_map(|step| match step {
                InstallStep::EnsureDirectory(path)
                    if !path.ends_with("/bin")
                        && !path.ends_with("/config")
                        && !path.ends_with("/plugin") =>
                {
                    Some(path.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| "/opt/rsagent".to_string());
        let script = self.render_steps(&install_root, steps, runtime_config, install_conf);
        let output = Self::build_ssh_command(connection, &script)
            .output()
            .await
            .map_err(|err| crate::NodeManageError::Storage(err.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(crate::NodeManageError::Storage(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepositoryRegistrationWaiter<R>
where
    R: NodeRepository,
{
    repository: R,
    poll_interval_secs: u64,
}

impl<R> RepositoryRegistrationWaiter<R>
where
    R: NodeRepository,
{
    pub fn new(repository: R, poll_interval_secs: u64) -> Self {
        Self {
            repository,
            poll_interval_secs,
        }
    }
}

#[async_trait]
impl<R> RegistrationWaiter for RepositoryRegistrationWaiter<R>
where
    R: NodeRepository,
{
    async fn wait_for_registration(&self, host: &str, timeout_secs: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));

        loop {
            let page = self.repository.list(PaginationParams::new(1, 100)).await?;
            if page
                .items
                .iter()
                .any(|node| node.endpoint.contains(host) || node.name.contains(host))
            {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(crate::NodeManageError::Storage(format!(
                    "timeout waiting for agent registration for host {host}"
                )));
            }

            sleep(Duration::from_secs(self.poll_interval_secs.max(1))).await;
        }
    }
}

#[derive(Clone)]
pub struct SshRsAgentInstaller {
    executor: Arc<dyn RemoteExecutor>,
    waiter: Arc<dyn RegistrationWaiter>,
    default_plugins: Vec<InstallPlugin>,
    wait_timeout_secs: u64,
}

impl SshRsAgentInstaller {
    pub fn new(
        executor: Arc<dyn RemoteExecutor>,
        waiter: Arc<dyn RegistrationWaiter>,
        default_plugins: Vec<InstallPlugin>,
        wait_timeout_secs: u64,
    ) -> Self {
        Self {
            executor,
            waiter,
            default_plugins,
            wait_timeout_secs,
        }
    }

    fn effective_plugins(&self, request: &InstallNodeRequest) -> Vec<InstallPlugin> {
        if request.plugins.is_empty() {
            self.default_plugins.clone()
        } else {
            request.plugins.clone()
        }
    }

    fn rsagent_version_from_url(url: &str) -> String {
        url.rsplit('/').next().unwrap_or(url).to_string()
    }
}

#[async_trait]
impl RsAgentInstaller for SshRsAgentInstaller {
    async fn install(&self, request: InstallNodeRequest) -> Result<InstallNodeResult> {
        let connection = SshConnectionRequest::from_parts(
            request.host.clone(),
            request.ssh_port,
            request.username.clone(),
            request.password.clone(),
            request.private_key.clone(),
        )?;

        let plugins = self.effective_plugins(&request);
        let existing_metadata = self
            .executor
            .read_install_conf(&connection, &request.install_root)
            .await?
            .as_deref()
            .map(InstallMetadata::from_install_conf)
            .transpose()?;

        let decision = InstallMetadata::decide(
            existing_metadata.as_ref(),
            &Self::rsagent_version_from_url(&request.rsagent_package_url),
            &plugins,
        );

        let runtime_config = InstallRuntimeConfig::new(
            request.install_root.clone(),
            request.register_callback_url.clone(),
        )
        .render()?;

        let install_conf = InstallMetadata {
            rsagent_version: Self::rsagent_version_from_url(&request.rsagent_package_url),
            plugins: plugins.clone(),
            status: InstallMetadataStatus::Installed,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
        .to_install_conf()?;

        let steps = match decision {
            InstallDecision::Skip => vec![InstallStep::WriteRuntimeConfig, InstallStep::StartAgent],
            _ => InstallStep::plan_for_fresh_install(
                request.install_root.clone(),
                request.rsagent_package_url.clone(),
                &plugins,
            ),
        };

        self.executor
            .execute_install_plan(&connection, &steps, &runtime_config, &install_conf)
            .await?;

        match self
            .waiter
            .wait_for_registration(&request.host, self.wait_timeout_secs)
            .await
        {
            Ok(()) => Ok(InstallNodeResult {
                install_id: Uuid::new_v4().to_string(),
                host: request.host,
                status: InstallStatus::Registered,
                message: None,
            }),
            Err(err) => Ok(InstallNodeResult {
                install_id: Uuid::new_v4().to_string(),
                host: request.host,
                status: InstallStatus::Failed,
                message: Some(err.to_string()),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMetadataStatus {
    Installing,
    Installed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallDecision {
    Install,
    Skip,
    Repair,
    Reinstall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallMetadata {
    pub rsagent_version: String,
    #[serde(default)]
    pub plugins: Vec<InstallPlugin>,
    pub status: InstallMetadataStatus,
    pub updated_at: String,
}

impl InstallMetadata {
    pub fn to_install_conf(&self) -> Result<String> {
        toml::to_string(self).map_err(|err| crate::NodeManageError::Storage(err.to_string()))
    }

    pub fn from_install_conf(raw: &str) -> Result<Self> {
        toml::from_str(raw).map_err(|err| crate::NodeManageError::Storage(err.to_string()))
    }

    pub fn decide(
        existing: Option<&Self>,
        required_rsagent_version: &str,
        required_plugins: &[InstallPlugin],
    ) -> InstallDecision {
        let Some(existing) = existing else {
            return InstallDecision::Install;
        };

        if existing.status != InstallMetadataStatus::Installed {
            return InstallDecision::Repair;
        }

        if existing.rsagent_version != required_rsagent_version {
            return InstallDecision::Reinstall;
        }

        let all_plugins_match = required_plugins.iter().all(|required| {
            existing
                .plugins
                .iter()
                .find(|installed| installed.name == required.name)
                .is_some_and(|installed| installed.version == required.version)
        });

        if all_plugins_match {
            InstallDecision::Skip
        } else {
            InstallDecision::Reinstall
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallNodeRequest {
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    pub rsagent_package_url: String,
    #[serde(default = "default_install_root")]
    pub install_root: String,
    #[serde(default = "default_register_callback_url")]
    pub register_callback_url: String,
    #[serde(default)]
    pub plugins: Vec<InstallPlugin>,
    #[serde(default)]
    pub labels: Vec<String>,
}

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallStatus {
    Pending,
    Installing,
    WaitingRegister,
    Registered,
    Failed,
}

impl InstallStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Installing => "installing",
            Self::WaitingRegister => "waiting_register",
            Self::Registered => "registered",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallNodeResult {
    pub install_id: String,
    pub host: String,
    pub status: InstallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl InstallNodeResult {
    pub fn pending(host: String) -> Self {
        Self {
            install_id: Uuid::new_v4().to_string(),
            host,
            status: InstallStatus::Pending,
            message: None,
        }
    }
}

#[async_trait]
pub trait RsAgentInstaller: Clone + Send + Sync + 'static {
    async fn install(&self, request: InstallNodeRequest) -> Result<InstallNodeResult>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopRsAgentInstaller;

#[async_trait]
impl RsAgentInstaller for NoopRsAgentInstaller {
    async fn install(&self, request: InstallNodeRequest) -> Result<InstallNodeResult> {
        Ok(InstallNodeResult::pending(request.host))
    }
}
