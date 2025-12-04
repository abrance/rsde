use tracing::{error, info, span, subscriber, warn, Level};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Debug)]
pub struct LogConfig {
    pub level: String,
    pub file_path: Option<String>,
}

const LOG_FILE_NAME: &str = "rsync.log";
const LOG_DIR_NAME: &str = "./log";

pub fn setup(config: LogConfig) {
    // 设置日志级别
    match config.level.as_str() {
        "debug" => println!("Log level set to DEBUG"),
        "info" => println!("Log level set to INFO"),
        "warn" => println!("Log level set to WARN"),
        "error" => println!("Log level set to ERROR"),
        _ => println!("Unknown log level, defaulting to INFO"),
    }

    let log_dir_path = String::from(LOG_DIR_NAME);
    if let Some(file_path) = config.file_path {
        println!("Log file path set to: {file_path}",);
    } else {
        println!("Using default log directory: {log_dir_path}",);
    }

    // 设置默认的 Subscriber （输出到 stdout）
    // tracing_subscriber::fmt::init();
    // 设置默认的 Subscriber （输出到 file 中）
    let file_appender = rolling::daily(log_dir_path, LOG_FILE_NAME);

    let (non_blocking, _guard) = non_blocking(file_appender);
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env()) // 支持 RUST_LOG
        .with_writer(non_blocking) // 输出到文件
        .json() // 使用 JSON 格式（推荐）
        .flatten_event(true) // 将 event 字段扁平化
        .with_current_span(true) // 包含当前 span
        .with_span_list(true) // 显示 span 层级
        .finish();
    subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");

    let span = span!(Level::INFO, "main", version = "1.0");
    let _enter = span.enter();
    info!("Starting Conquer application...");
    warn!("This is a warning message");
    error!("This is an error message");
}
