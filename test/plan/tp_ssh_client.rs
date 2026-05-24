use std::{
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::fs;
use util::client::ssh::SshClientConfig;

fn test_host() -> String {
    env::var("TEST_SSH_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn test_port() -> u16 {
    env::var("TEST_SSH_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(22)
}

fn test_username() -> String {
    env::var("TEST_SSH_USERNAME").expect("TEST_SSH_USERNAME is required for SSH integration tests")
}

fn build_password_config() -> SshClientConfig {
    let password = env::var("TEST_SSH_PASSWORD")
        .expect("TEST_SSH_PASSWORD is required for password-based SSH integration tests");

    SshClientConfig::new(format!("{}:{}", test_host(), test_port()), test_username())
        .with_password(password)
        .with_disable_host_key_check(true)
}

fn build_private_key_config() -> SshClientConfig {
    let private_key_path = env::var("TEST_SSH_PRIVATE_KEY")
        .expect("TEST_SSH_PRIVATE_KEY is required for private-key SSH integration tests");

    let config = SshClientConfig::new(format!("{}:{}", test_host(), test_port()), test_username())
        .with_private_key_path(private_key_path)
        .with_disable_host_key_check(true);

    if let Ok(passphrase) = env::var("TEST_SSH_PRIVATE_KEY_PASSPHRASE") {
        config.with_private_key_passphrase(passphrase)
    } else {
        config
    }
}

fn unique_remote_path(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();

    format!("/tmp/{}_{}", prefix, nanos)
}

#[tokio::test]
#[ignore]
async fn test_ssh_password_exec() {
    let client = util::client::ssh::SshClient::new(&build_password_config())
        .await
        .expect("create SSH client");

    let result = client
        .exec("echo hello-ssh")
        .await
        .expect("run remote command");

    assert_eq!(result.stdout.trim(), "hello-ssh");
    assert_eq!(result.exit_status, 0);
}

#[tokio::test]
#[ignore]
async fn test_ssh_ping() {
    let client = util::client::ssh::SshClient::new(&build_password_config())
        .await
        .expect("create SSH client");

    client.ping().await.expect("SSH ping should succeed");
}

#[tokio::test]
#[ignore]
async fn test_ssh_private_key_exec() {
    let client = util::client::ssh::SshClient::new(&build_private_key_config())
        .await
        .expect("create SSH client");

    let result = client
        .exec("printf private-key-auth")
        .await
        .expect("run remote command");

    assert_eq!(result.stdout.trim(), "private-key-auth");
    assert_eq!(result.exit_status, 0);
}

#[tokio::test]
#[ignore]
async fn test_upload_file() {
    let client = util::client::ssh::SshClient::new(&build_password_config())
        .await
        .expect("create SSH client");

    let local_path = PathBuf::from(format!(
        "/tmp/rsde-ssh-upload-{}.txt",
        unique_remote_path("local").replace('/', "_")
    ));
    let remote_path = unique_remote_path("rsde-ssh-upload");
    let expected = "uploaded-from-rsde";

    fs::write(&local_path, expected)
        .await
        .expect("write local upload fixture");

    client
        .upload(&local_path, &remote_path)
        .await
        .expect("upload should succeed");

    let verify = client
        .exec(&format!("cat {}", remote_path))
        .await
        .expect("read uploaded remote file");

    assert_eq!(verify.stdout, expected);

    let _ = client.exec(&format!("rm -f {}", remote_path)).await;
    let _ = fs::remove_file(&local_path).await;
}

#[tokio::test]
#[ignore]
async fn test_download_file() {
    let client = util::client::ssh::SshClient::new(&build_password_config())
        .await
        .expect("create SSH client");

    let remote_path = unique_remote_path("rsde-ssh-download");
    let local_path = PathBuf::from(format!(
        "/tmp/rsde-ssh-download-{}.txt",
        unique_remote_path("local").replace('/', "_")
    ));
    let expected = "downloaded-from-rsde";

    client
        .exec(&format!("printf '{}' > {}", expected, remote_path))
        .await
        .expect("prepare remote fixture");

    client
        .download(&remote_path, &local_path)
        .await
        .expect("download should succeed");

    let contents = fs::read_to_string(&local_path)
        .await
        .expect("read downloaded local file");
    assert_eq!(contents, expected);

    let _ = client.exec(&format!("rm -f {}", remote_path)).await;
    let _ = fs::remove_file(&local_path).await;
}
