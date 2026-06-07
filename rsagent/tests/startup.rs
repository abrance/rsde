use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rsagent::config::AUTO_PROVISION_AGENT_ID;

#[test]
fn rsagent_binary_honors_config_cli_argument_for_installed_startup() {
    let sandbox = std::env::temp_dir().join(format!(
        "rsagent-startup-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&sandbox).expect("create sandbox");

    let data_dir = sandbox.join("data");
    let config_path = sandbox.join("rsagent.toml");
    fs::write(
        &config_path,
        format!(
            "nodemanage_sync_url = \"http://127.0.0.1:9/agent/sync\"\nagent_id = \"{AUTO_PROVISION_AGENT_ID}\"\ndata_dir = \"{}\"\n",
            data_dir.display()
        ),
    )
    .expect("write rsagent config");

    let mut child = Command::new(env!("CARGO_BIN_EXE_rsagent"))
        .arg("--config")
        .arg(&config_path)
        .env_remove("RSAGENT_CONFIG")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rsagent binary");

    let identity_path = data_dir.join("agent-identity.toml");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;

    while Instant::now() < deadline {
        if identity_path.exists() {
            found = true;
            break;
        }

        if let Some(status) = child.try_wait().expect("poll rsagent process") {
            panic!(
                "rsagent exited before persisting identity to {}: {status}",
                identity_path.display()
            );
        }

        thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        found,
        "expected rsagent started with --config to persist identity at {}",
        identity_path.display()
    );

    let persisted = fs::read_to_string(&identity_path).expect("read persisted identity");
    assert!(persisted.contains("agent_id"));
    assert!(!persisted.contains(AUTO_PROVISION_AGENT_ID));

    let _ = fs::remove_dir_all(&sandbox);
}
