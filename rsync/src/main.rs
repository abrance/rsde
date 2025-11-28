use rule::{controller::Controller, rule_file_watch::RuleFileWatcher};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));

    // 启动文件监听器
    let controller_clone = controller.clone();
    tokio::spawn(async move {
        println!("Starting file watcher in current directory...");
        let mut watcher = RuleFileWatcher::new(controller_clone, ".".to_string());
        watcher.run().await;
    });

    // Keep the main thread alive to let the controller run
    println!("Rsync service running... Waiting for config files in current directory.");
    println!("Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.unwrap();
    println!("Shutdown signal received");
}
