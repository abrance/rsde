use axum::{http::StatusCode, response::Json, routing::get, Router};
use rule::rule::GlobalConfigData;
use rule::{controller::Controller, rule_file_watch::RuleFileWatcher};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use util::log::setup;

const DEFAULT_CONFIG_FILE_LIST: [&str; 3] = ["config.toml", "rsync.toml", "example.toml"];

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";
async fn health_check() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "status": "healthy" })))
}

#[tokio::main]
async fn main() {
    // 首先尝试从默认配置文件加载全局配置
    let global_config = load_global_config_from_file().unwrap_or_else(|_| {
        // 如果无法加载配置文件，则使用默认配置
        GlobalConfigData::default()
    });

    // 从全局配置中提取日志配置
    let log_config = util::log::LogConfig {
        level: global_config.api.log_level.clone(),
        file_path: Some(global_config.log.path.clone()),
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
    let app = Router::new().route("/health", get(health_check));

    let listener = tokio::net::TcpListener::bind(DEFAULT_LISTEN_ADDR)
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

fn load_global_config_from_file() -> Result<GlobalConfigData, Box<dyn std::error::Error>> {
    // 尝试加载全局配置文件
    let config_path = DEFAULT_CONFIG_FILE_LIST.iter().find_map(|&file| {
        std::path::Path::new(file)
            .exists()
            .then(|| file.to_string())
    });

    if let Some(path) = config_path {
        // 使用新的 GlobalConfigData::from_file 方法加载全局配置
        let global_config = GlobalConfigData::from_file(path)?;
        Ok(global_config)
    } else {
        // 如果没有找到配置文件，返回错误
        Err("No configuration file found".into())
    }
}
