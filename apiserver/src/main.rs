mod image;
mod ocr;

use axum::{Router, http::StatusCode};
use config::{ConfigLoader, GlobalConfig};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
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
    let image_hosting_config = global_config
        .image_hosting
        .expect("配置文件中缺少 [image_hosting] 部分");
    info!("配置加载成功");

    // 启动图片清理任务
    image::start_cleanup_task(image_hosting_config.clone());

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 前端静态文件目录
    let frontend_dir = "webserver/frontend/dist";
    let index_file = format!("{frontend_dir}/index.html");

    // 检查前端文件是否存在
    let has_frontend = std::path::Path::new(&frontend_dir).exists();
    if !has_frontend {
        info!("⚠️  前端文件未找到: {frontend_dir}");
        info!("   运行 'cd webserver/frontend && npm run build' 构建前端");
    }

    // 构建路由
    let mut app = Router::new()
        // API 路由
        .nest(
            "/api/ocr",
            ocr::create_routes(remote_ocr_config, image_hosting_config.storage_dir.clone()),
        )
        .nest("/api/image", image::create_routes(image_hosting_config));

    // 如果前端文件存在，添加静态文件服务
    if has_frontend {
        app = app
            .nest_service("/assets", ServeDir::new(format!("{frontend_dir}/assets")))
            .fallback_service(
                ServeDir::new(frontend_dir).not_found_service(ServeFile::new(&index_file)),
            );
        info!("✅ 前端服务已启用");
    } else {
        // 如果前端不存在，提供一个简单的说明页面
        app = app.fallback(|| async {
            (
                StatusCode::OK,
                "RSDE API Server\n\n前端未构建，请运行:\ncd webserver/frontend && npm install && npm run build"
            )
        });
    }

    // 添加中间件
    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(cors),
    );

    // 监听地址
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("服务器监听地址: {}", addr);
    info!("API 接口:");
    info!("  POST http://localhost:3000/api/ocr/single_pic - 远程 OCR");
    info!("  GET  http://localhost:3000/api/ocr/health - 健康检查");
    info!("  POST http://localhost:3000/api/image/upload - 图片上传");
    if has_frontend {
        info!("前端页面:");
        info!("  http://localhost:3000/ - Web UI");
    }

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
