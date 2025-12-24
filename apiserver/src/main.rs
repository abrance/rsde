mod ocr;

use axum::Router;
use config::{ConfigLoader, GlobalConfig};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "apiserver=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("启动 API Server...");

    // 加载配置文件
    let config_path =
        std::env::var("API_CONFIG").unwrap_or_else(|_| "apiserver/config.toml".to_string());
    info!("加载配置文件: {}", config_path);
    let global_config = GlobalConfig::from_file(&config_path)?;
    let remote_ocr_config = global_config
        .remote_ocr
        .expect("配置文件中缺少 [remote_ocr] 部分");
    info!("配置加载成功");

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 构建路由
    let app = Router::new()
        .nest("/ocr", ocr::create_routes(remote_ocr_config))
        .layer(cors);

    // 监听地址
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("服务器监听地址: {}", addr);
    info!("OCR API 可用:");
    info!("  POST http://localhost:3000/ocr/single_pic - 远程 OCR");
    info!("  POST http://localhost:3000/ocr/single_pic_local - 本地 OCR");
    info!("  GET  http://localhost:3000/ocr/health - 健康检查");

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
