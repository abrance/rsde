use nodemanage::{
    InstallDecision, InstallMetadata, InstallMetadataStatus, InstallNodeRequest, InstallPlugin,
    InstallRuntimeConfig, InstallStatus, InstallStep, RegistrationWaiter, RemoteExecutor,
    RsAgentInstaller, SshAuth, SshConnectionRequest, SshRsAgentInstaller,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingExecutor {
    calls: Arc<Mutex<Vec<(SshConnectionRequest, Vec<InstallStep>, String, String)>>>,
    existing_install_conf: Option<String>,
}

#[async_trait::async_trait]
impl RemoteExecutor for RecordingExecutor {
    async fn read_install_conf(
        &self,
        _connection: &SshConnectionRequest,
        _install_root: &str,
    ) -> nodemanage::Result<Option<String>> {
        Ok(self.existing_install_conf.clone())
    }

    async fn execute_install_plan(
        &self,
        connection: &SshConnectionRequest,
        steps: &[InstallStep],
        runtime_config: &str,
        install_conf: &str,
    ) -> nodemanage::Result<()> {
        self.calls.lock().unwrap().push((
            connection.clone(),
            steps.to_vec(),
            runtime_config.to_string(),
            install_conf.to_string(),
        ));
        Ok(())
    }
}

#[derive(Clone)]
struct StaticWaiter {
    result: nodemanage::Result<()>,
}

#[async_trait::async_trait]
impl RegistrationWaiter for StaticWaiter {
    async fn wait_for_registration(
        &self,
        _host: &str,
        _timeout_secs: u64,
    ) -> nodemanage::Result<()> {
        self.result.clone()
    }
}

#[test]
fn install_metadata_round_trips_install_conf_shape() {
    let metadata = InstallMetadata {
        rsagent_version: "1.2.3".to_string(),
        plugins: vec![InstallPlugin {
            name: "metrics".to_string(),
            version: "2.0.0".to_string(),
            package_url: Some("https://example.com/plugins/metrics.tar.gz".to_string()),
        }],
        status: InstallMetadataStatus::Installed,
        updated_at: "2026-05-25T12:00:00Z".to_string(),
    };

    let encoded = metadata.to_install_conf().unwrap();
    let decoded = InstallMetadata::from_install_conf(&encoded).unwrap();

    assert_eq!(decoded.rsagent_version, "1.2.3");
    assert_eq!(decoded.plugins.len(), 1);
    assert_eq!(decoded.plugins[0].name, "metrics");
    assert_eq!(decoded.status, InstallMetadataStatus::Installed);
}

#[test]
fn missing_install_conf_requires_fresh_install() {
    let decision = InstallMetadata::decide(None, "1.2.3", &[]);
    assert_eq!(decision, InstallDecision::Install);
}

#[test]
fn successful_matching_install_conf_skips_reinstall() {
    let existing = InstallMetadata {
        rsagent_version: "1.2.3".to_string(),
        plugins: vec![InstallPlugin {
            name: "metrics".to_string(),
            version: "2.0.0".to_string(),
            package_url: None,
        }],
        status: InstallMetadataStatus::Installed,
        updated_at: "2026-05-25T12:00:00Z".to_string(),
    };
    let required_plugins = vec![InstallPlugin {
        name: "metrics".to_string(),
        version: "2.0.0".to_string(),
        package_url: None,
    }];

    let decision = InstallMetadata::decide(Some(&existing), "1.2.3", &required_plugins);
    assert_eq!(decision, InstallDecision::Skip);
}

#[test]
fn failed_install_conf_requires_repair() {
    let existing = InstallMetadata {
        rsagent_version: "1.2.3".to_string(),
        plugins: vec![],
        status: InstallMetadataStatus::Failed,
        updated_at: "2026-05-25T12:00:00Z".to_string(),
    };

    let decision = InstallMetadata::decide(Some(&existing), "1.2.3", &[]);
    assert_eq!(decision, InstallDecision::Repair);
}

#[test]
fn version_mismatch_requires_reinstall() {
    let existing = InstallMetadata {
        rsagent_version: "1.0.0".to_string(),
        plugins: vec![InstallPlugin {
            name: "metrics".to_string(),
            version: "1.0.0".to_string(),
            package_url: None,
        }],
        status: InstallMetadataStatus::Installed,
        updated_at: "2026-05-25T12:00:00Z".to_string(),
    };
    let required_plugins = vec![InstallPlugin {
        name: "metrics".to_string(),
        version: "2.0.0".to_string(),
        package_url: None,
    }];

    let decision = InstallMetadata::decide(Some(&existing), "1.2.3", &required_plugins);
    assert_eq!(decision, InstallDecision::Reinstall);
}

#[test]
fn install_status_exposes_waiting_register_state() {
    assert_eq!(InstallStatus::WaitingRegister.as_str(), "waiting_register");
}

#[tokio::test]
async fn ssh_installer_executes_plan_and_returns_registered_on_successful_wait() {
    let executor = RecordingExecutor::default();
    let calls = executor.calls.clone();
    let installer = SshRsAgentInstaller::new(
        Arc::new(executor),
        Arc::new(StaticWaiter { result: Ok(()) }),
        vec![InstallPlugin {
            name: "metrics".to_string(),
            version: "1.0.0".to_string(),
            package_url: Some("https://example.com/plugins/metrics.tar.gz".to_string()),
        }],
        30,
    );

    let result = installer
        .install(InstallNodeRequest {
            host: "10.0.0.8".to_string(),
            ssh_port: 22,
            username: "root".to_string(),
            password: Some("secret".to_string()),
            private_key: None,
            rsagent_package_url: "https://example.com/rsagent-1.2.3.tar.gz".to_string(),
            install_root: "/opt/rsagent".to_string(),
            register_callback_url: "http://127.0.0.1:3000/api/nodes/agent/register".to_string(),
            plugins: vec![],
            labels: vec![],
        })
        .await
        .unwrap();

    assert_eq!(result.status, InstallStatus::Registered);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].2.contains("register_callback_url"));
    assert!(
        calls[0]
            .3
            .contains("rsagent_version = \"rsagent-1.2.3.tar.gz\"")
    );
}

#[tokio::test]
async fn ssh_installer_returns_failed_when_registration_wait_fails() {
    let installer = SshRsAgentInstaller::new(
        Arc::new(RecordingExecutor::default()),
        Arc::new(StaticWaiter {
            result: Err(nodemanage::NodeManageError::Storage("timeout".to_string())),
        }),
        vec![],
        30,
    );

    let result = installer
        .install(InstallNodeRequest {
            host: "10.0.0.8".to_string(),
            ssh_port: 22,
            username: "root".to_string(),
            password: Some("secret".to_string()),
            private_key: None,
            rsagent_package_url: "https://example.com/rsagent-1.2.3.tar.gz".to_string(),
            install_root: "/opt/rsagent".to_string(),
            register_callback_url: "http://127.0.0.1:3000/api/nodes/agent/register".to_string(),
            plugins: vec![],
            labels: vec![],
        })
        .await
        .unwrap();

    assert_eq!(result.status, InstallStatus::Failed);
    assert!(result.message.unwrap().contains("timeout"));
}

#[test]
fn ssh_connection_request_prefers_password_when_present() {
    let request = SshConnectionRequest::from_parts(
        "10.0.0.8".to_string(),
        22,
        "root".to_string(),
        Some("secret".to_string()),
        Some("/tmp/id_rsa".to_string()),
    )
    .unwrap();

    assert_eq!(request.auth, SshAuth::Password("secret".to_string()));
}

#[test]
fn ssh_connection_request_uses_private_key_when_password_absent() {
    let request = SshConnectionRequest::from_parts(
        "10.0.0.8".to_string(),
        22,
        "root".to_string(),
        None,
        Some("/tmp/id_rsa".to_string()),
    )
    .unwrap();

    assert_eq!(request.auth, SshAuth::PrivateKey("/tmp/id_rsa".to_string()));
}

#[test]
fn runtime_config_contains_registration_target() {
    let config = InstallRuntimeConfig::new(
        "/opt/rsagent".to_string(),
        "http://127.0.0.1:3000/api/nodes/agent/register".to_string(),
    );

    let rendered = config.render().unwrap();
    assert!(rendered.contains("install_root = \"/opt/rsagent\""));
    assert!(
        rendered
            .contains("register_callback_url = \"http://127.0.0.1:3000/api/nodes/agent/register\"")
    );
}

#[test]
fn install_plan_creates_default_layout_and_writes_install_conf_before_start() {
    let plugins = vec![InstallPlugin {
        name: "metrics".to_string(),
        version: "1.0.0".to_string(),
        package_url: Some("https://example.com/plugins/metrics.tar.gz".to_string()),
    }];

    let steps = InstallStep::plan_for_fresh_install(
        "/opt/rsagent".to_string(),
        "https://example.com/rsagent.tar.gz".to_string(),
        &plugins,
    );

    assert_eq!(
        steps[0],
        InstallStep::EnsureDirectory("/opt/rsagent".to_string())
    );
    assert!(steps.contains(&InstallStep::EnsureDirectory(
        "/opt/rsagent/bin".to_string()
    )));
    assert!(steps.contains(&InstallStep::EnsureDirectory(
        "/opt/rsagent/config".to_string()
    )));
    assert!(steps.contains(&InstallStep::EnsureDirectory(
        "/opt/rsagent/plugin".to_string()
    )));
    assert!(steps.contains(&InstallStep::DownloadRsAgent(
        "https://example.com/rsagent.tar.gz".to_string()
    )));
    assert!(steps.contains(&InstallStep::DownloadPlugin {
        name: "metrics".to_string(),
        package_url: "https://example.com/plugins/metrics.tar.gz".to_string(),
    }));

    let install_conf_index = steps
        .iter()
        .position(|step| matches!(step, InstallStep::WriteInstallConf))
        .unwrap();
    let start_index = steps
        .iter()
        .position(|step| matches!(step, InstallStep::StartAgent))
        .unwrap();
    assert!(install_conf_index < start_index);
}
