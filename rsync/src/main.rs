use rule::{controller::Controller, rule_file_watch::RuleFileWatcher};
use std::sync::Arc;
use tokio::sync::Mutex;
use util::log::info;

#[tokio::main]
async fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));

    // 启动文件监听器
    let controller_clone = controller.clone();
    tokio::spawn(async move {
        let watch_dir = std::env::var("RSYNC_CONFIG_DIR").unwrap_or_else(|_| ".".to_string());
        info!("Starting file watcher in directory: {watch_dir}");
        let mut watcher = RuleFileWatcher::new(controller_clone, watch_dir);
        watcher.run().await;
    });

    // Keep the main thread alive to let the controller run
    info!("Rsync service running... Waiting for config files in current directory.");
    info!("Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.unwrap();
    info!("Shutdown signal received");
}
