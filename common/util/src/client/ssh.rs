//! SSH 客户端封装
//!
//! 提供 SSH 连接和基础操作的封装，支持：
//! - 密码认证
//! - 私钥认证
//! - 远程命令执行
//! - 路径型文件上传下载
//!
//! # 注意
//!
//! 第一版按需求默认不做 host key 校验，这只适合受控环境，不能视为安全默认值。

use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, sync::Arc, time::Duration};

use russh::{
    ChannelMsg, Disconnect,
    client::{self, Handle},
    keys::{PrivateKeyWithHashAlg, load_secret_key, ssh_key},
};
use russh_sftp::client::SftpSession;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, copy},
};

/// SSH 客户端配置
#[derive(Clone, Serialize, Deserialize)]
pub struct SshClientConfig {
    /// SSH 服务器主机名或地址
    pub host: String,
    /// SSH 端口（默认：22）
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码认证
    #[serde(skip_serializing)]
    pub password: Option<String>,
    /// 私钥文件路径
    pub private_key_path: Option<String>,
    /// 私钥口令
    #[serde(skip_serializing)]
    pub private_key_passphrase: Option<String>,
    /// 连接超时时间（秒）
    pub timeout: Option<u64>,
    /// 是否关闭 host key 校验
    pub disable_host_key_check: bool,
}

impl SshClientConfig {
    /// 创建一个新的 SSH 客户端配置
    pub fn new(host: impl Into<String>, username: impl Into<String>) -> Self {
        let host_str = host.into();
        let (host, port) = if let Some(pos) = host_str.rfind(':') {
            let host_part = &host_str[..pos];
            let port_part = &host_str[pos + 1..];
            match port_part.parse::<u16>() {
                Ok(port) if !host_part.is_empty() => (host_part.to_string(), port),
                _ => (host_str, 22),
            }
        } else {
            (host_str, 22)
        };

        Self {
            host,
            port,
            username: username.into(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
            timeout: Some(10),
            disable_host_key_check: true,
        }
    }

    /// 设置端口
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// 设置密码
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// 设置私钥路径
    pub fn with_private_key_path(mut self, path: impl Into<String>) -> Self {
        self.private_key_path = Some(path.into());
        self
    }

    /// 设置私钥口令
    pub fn with_private_key_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.private_key_passphrase = Some(passphrase.into());
        self
    }

    /// 设置超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置是否关闭 host key 校验
    pub fn with_disable_host_key_check(mut self, disable: bool) -> Self {
        self.disable_host_key_check = disable;
        self
    }
}

impl fmt::Debug for SshClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshClientConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("private_key_path", &self.private_key_path)
            .field(
                "private_key_passphrase",
                &self.private_key_passphrase.as_ref().map(|_| "<redacted>"),
            )
            .field("timeout", &self.timeout)
            .field("disable_host_key_check", &self.disable_host_key_check)
            .finish()
    }
}

/// 远程命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_status: i32,
}

/// SSH 客户端
pub struct SshClient {
    config: SshClientConfig,
}

impl SshClient {
    /// 创建一个新的 SSH 客户端
    pub async fn new(config: &SshClientConfig) -> Result<Self, String> {
        Self::validate_config(config)?;

        Ok(Self {
            config: config.clone(),
        })
    }

    /// 验证配置是否合法
    pub(crate) fn validate_config(config: &SshClientConfig) -> Result<(), String> {
        let has_password = config.password.is_some();
        let has_private_key = config.private_key_path.is_some();

        match (has_password, has_private_key) {
            (false, false) => Err("SSH authentication is required".to_string()),
            (true, true) => {
                Err("SSH client requires exactly one authentication method in v1".to_string())
            }
            (_, _) if !config.disable_host_key_check => Err(
                "Host key verification is not supported in v1; disable_host_key_check must remain true"
                    .to_string(),
            ),
            _ => Ok(()),
        }
    }

    /// 获取配置信息
    pub fn get_config(&self) -> &SshClientConfig {
        &self.config
    }

    /// 检查 SSH 连接
    pub async fn ping(&self) -> Result<(), String> {
        let mut session = self.connect_authenticated().await?;
        self.disconnect_session(&mut session).await?;
        Ok(())
    }

    /// 执行远程命令
    pub async fn exec(&self, command: &str) -> Result<SshExecResult, String> {
        let mut session = self.connect_authenticated().await?;
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| format!("Failed to open SSH session channel: {e}"))?;

        channel
            .exec(true, command)
            .await
            .map_err(|e| format!("Failed to execute SSH command '{command}': {e}"))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_status = None;

        loop {
            let Some(msg) = channel.wait().await else {
                break;
            };

            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext: 1 } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus {
                    exit_status: status,
                } => exit_status = Some(status as i32),
                ChannelMsg::Close => break,
                _ => {}
            }
        }

        channel
            .close()
            .await
            .map_err(|e| format!("Failed to close SSH command channel: {e}"))?;
        self.disconnect_session(&mut session).await?;

        Ok(SshExecResult {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            exit_status: exit_status
                .ok_or_else(|| "SSH command did not return an exit status".to_string())?,
        })
    }

    /// 上传文件到远端
    ///
    /// 如果传输中断，远端可能会留下部分写入的目标文件。
    pub async fn upload(
        &self,
        local_path: impl AsRef<Path>,
        remote_path: impl AsRef<Path>,
    ) -> Result<(), String> {
        let local_path = local_path.as_ref();
        let remote_path = remote_path.as_ref();

        let mut local_file = File::open(local_path)
            .await
            .map_err(|e| format!("Failed to open local file '{}': {e}", local_path.display()))?;

        let mut session = self.connect_authenticated().await?;
        let sftp = self.open_sftp(&session).await?;
        let mut remote_file = sftp
            .create(remote_path.to_string_lossy().into_owned())
            .await
            .map_err(|e| {
                format!(
                    "Failed to create remote file '{}': {e}",
                    remote_path.display()
                )
            })?;

        copy(&mut local_file, &mut remote_file)
            .await
            .map_err(|e| format!("Failed to upload file to '{}': {e}", remote_path.display()))?;

        remote_file.flush().await.map_err(|e| {
            format!(
                "Failed to flush remote file '{}': {e}",
                remote_path.display()
            )
        })?;
        remote_file.shutdown().await.map_err(|e| {
            format!(
                "Failed to close remote file '{}': {e}",
                remote_path.display()
            )
        })?;

        drop(remote_file);
        drop(sftp);
        self.disconnect_session(&mut session).await?;
        Ok(())
    }

    /// 从远端下载文件
    ///
    /// 如果传输中断，本地可能会留下部分写入的目标文件。
    pub async fn download(
        &self,
        remote_path: impl AsRef<Path>,
        local_path: impl AsRef<Path>,
    ) -> Result<(), String> {
        let remote_path = remote_path.as_ref();
        let local_path = local_path.as_ref();

        let mut session = self.connect_authenticated().await?;
        let sftp = self.open_sftp(&session).await?;
        let mut remote_file = sftp
            .open(remote_path.to_string_lossy().into_owned())
            .await
            .map_err(|e| {
                format!(
                    "Failed to open remote file '{}': {e}",
                    remote_path.display()
                )
            })?;
        let mut local_file = File::create(local_path).await.map_err(|e| {
            format!(
                "Failed to create local file '{}': {e}",
                local_path.display()
            )
        })?;

        copy(&mut remote_file, &mut local_file)
            .await
            .map_err(|e| format!("Failed to download file '{}': {e}", remote_path.display()))?;

        local_file
            .flush()
            .await
            .map_err(|e| format!("Failed to flush local file '{}': {e}", local_path.display()))?;

        drop(local_file);
        drop(remote_file);
        drop(sftp);
        self.disconnect_session(&mut session).await?;
        Ok(())
    }

    async fn connect_authenticated(&self) -> Result<Handle<NoCheckHandler>, String> {
        Self::validate_config(&self.config)?;

        let timeout = self.config.timeout.map(Duration::from_secs);
        let client_config = client::Config {
            inactivity_timeout: timeout,
            ..Default::default()
        };

        let mut session = client::connect(
            Arc::new(client_config),
            (self.config.host.as_str(), self.config.port),
            NoCheckHandler,
        )
        .await
        .map_err(|e| {
            format!(
                "Failed to connect to SSH server {}:{}: {e}",
                self.config.host, self.config.port
            )
        })?;

        let auth_result = if let Some(password) = &self.config.password {
            session
                .authenticate_password(self.config.username.clone(), password.clone())
                .await
                .map_err(|e| format!("SSH password authentication failed: {e}"))?
        } else if let Some(private_key_path) = &self.config.private_key_path {
            let key_pair = load_secret_key(
                private_key_path,
                self.config.private_key_passphrase.as_deref(),
            )
            .map_err(|e| format!("Failed to load private key '{private_key_path}': {e}"))?;

            let hash_alg = session
                .best_supported_rsa_hash()
                .await
                .map_err(|e| format!("Failed to determine supported RSA hash algorithm: {e}"))?
                .flatten();

            session
                .authenticate_publickey(
                    self.config.username.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
                )
                .await
                .map_err(|e| format!("SSH private key authentication failed: {e}"))?
        } else {
            return Err("SSH authentication is required".to_string());
        };

        if !auth_result.success() {
            return Err(format!(
                "SSH authentication was rejected for user '{}'",
                self.config.username
            ));
        }

        Ok(session)
    }

    async fn open_sftp(&self, session: &Handle<NoCheckHandler>) -> Result<SftpSession, String> {
        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| format!("Failed to open SSH session channel for SFTP: {e}"))?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("Failed to request SSH SFTP subsystem: {e}"))?;

        SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("Failed to initialize SFTP session: {e}"))
    }

    async fn disconnect_session(&self, session: &mut Handle<NoCheckHandler>) -> Result<(), String> {
        session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .map_err(|e| format!("Failed to disconnect SSH session cleanly: {e}"))
    }
}

#[derive(Clone, Copy, Debug)]
struct NoCheckHandler;

impl client::Handler for NoCheckHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::{SshClient, SshClientConfig, SshExecResult};
    use std::path::PathBuf;

    #[test]
    fn test_config_creation_defaults() {
        let config = SshClientConfig::new("example.com:2222", "alice");

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 2222);
        assert_eq!(config.username, "alice");
        assert_eq!(config.password, None);
        assert_eq!(config.private_key_path, None);
        assert_eq!(config.private_key_passphrase, None);
        assert_eq!(config.timeout, Some(10));
        assert!(config.disable_host_key_check);
    }

    #[test]
    fn test_config_with_default_port() {
        let config = SshClientConfig::new("example.com", "alice");

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 22);
    }

    #[test]
    fn test_config_with_password() {
        let config = SshClientConfig::new("example.com", "alice")
            .with_password("secret")
            .with_timeout(30)
            .with_disable_host_key_check(true);

        assert_eq!(config.password, Some("secret".to_string()));
        assert_eq!(config.timeout, Some(30));
        assert!(config.disable_host_key_check);
    }

    #[test]
    fn test_config_with_private_key() {
        let config = SshClientConfig::new("example.com", "alice")
            .with_private_key_path("/tmp/id_rsa")
            .with_private_key_passphrase("phrase");

        assert_eq!(config.private_key_path, Some("/tmp/id_rsa".to_string()));
        assert_eq!(config.private_key_passphrase, Some("phrase".to_string()));
    }

    #[test]
    fn test_new_rejects_host_key_verification_mode() {
        let config = SshClientConfig::new("example.com", "alice")
            .with_password("secret")
            .with_disable_host_key_check(false);

        let error = SshClient::validate_config(&config)
            .expect_err("host key verification should be rejected in v1");

        assert!(error.contains("Host key verification is not supported in v1"));
    }

    #[test]
    fn test_config_debug_redacts_secrets() {
        let config = SshClientConfig::new("example.com", "alice")
            .with_password("secret-token")
            .with_private_key_passphrase("passphrase-secret-token");

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-token"));
        assert!(!debug.contains("passphrase-secret-token"));
    }

    #[test]
    fn test_exec_result_serde_roundtrip() {
        let result = SshExecResult {
            stdout: "ok".to_string(),
            stderr: String::new(),
            exit_status: 0,
        };

        let json = serde_json::to_string(&result).expect("serialize exec result");
        let restored: SshExecResult = serde_json::from_str(&json).expect("deserialize exec result");

        assert_eq!(restored.stdout, "ok");
        assert_eq!(restored.stderr, "");
        assert_eq!(restored.exit_status, 0);
    }

    #[test]
    fn test_new_rejects_missing_auth() {
        let config = SshClientConfig::new("example.com", "alice");
        let error = SshClient::validate_config(&config).expect_err("missing auth should fail");

        assert!(error.contains("authentication"));
    }

    #[test]
    fn test_new_rejects_conflicting_auth() {
        let config = SshClientConfig::new("example.com", "alice")
            .with_password("secret")
            .with_private_key_path("/tmp/id_rsa");

        let error = SshClient::validate_config(&config).expect_err("conflicting auth should fail");

        assert!(error.contains("exactly one authentication"));
    }

    #[tokio::test]
    async fn test_upload_rejects_missing_local_file() {
        let client =
            SshClient::new(&SshClientConfig::new("example.com", "alice").with_password("secret"))
                .await
                .expect("client skeleton should build");

        let missing = PathBuf::from("/tmp/rsde-missing-ssh-upload-file");
        let error = client
            .upload(&missing, "/tmp/remote-file")
            .await
            .expect_err("missing local file should fail");

        assert!(error.contains("Failed to open local file"));
    }
}
