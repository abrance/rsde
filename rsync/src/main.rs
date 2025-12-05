use axum::{
    routing::get,
    Router,
    response::Json,
    http::StatusCode,
};
use rule::{controller::Controller, rule_file_watch::RuleFileWatcher};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use util::log::{setup, LogConfig};

async fn health_check() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "status": "healthy" })))
}

#[tokio::main]
async fn main() {
    let log_config = LogConfig {
        level: if std::env::var("DEBUG").is_ok() {
            "debug".to_string()
        } else {
            "info".to_string()
        },
        file_path: None,
    };
    setup(log_config);
    let controller = Arc::new(Mutex::new(Controller::new()));

    // 启动文件监听器
    let controller_clone = controller.clone();
    tokio::spawn(async move {
        let watch_dir = std::env::var("RSYNC_CONFIG_DIR").unwrap_or_else(|_| ".".to_string());
        info!("Starting file watcher in directory: {watch_dir}");
        let mut watcher = RuleFileWatcher::new(controller_clone, watch_dir);
        watcher.run().await;
    });

    // 启动 HTTP 服务
    let app = Router::new()
        .route("/health", get(health_check));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    info!("HTTP server running on port 8080");

    // 在单独的任务中运行 HTTP 服务器
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Failed to start HTTP server");
    });

    // Keep the main thread alive to let the controller run
    info!("Rsync service running... Waiting for config files in current directory.");
    info!("Health check endpoint available at http://localhost:8080/health");
    info!("Press Ctrl+C to stop.");

    // 等待 Ctrl+C 信号
    tokio::signal::ctrl_c().await.unwrap();
    info!("Shutdown signal received");

    // 取消 HTTP 服务器任务
    server_task.abort();
}
