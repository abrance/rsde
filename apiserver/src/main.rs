mod anybox;
mod image;
mod ocr;

use axum::Router;
use config::{ConfigLoader, GlobalConfig};
use prometheus::{Encoder, TextEncoder};
use std::{
    net::{IpAddr, SocketAddr},
    panic,
    path::Path,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Metrics handler - 导出所有 prometheus 指标
async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

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
    let anybox_config = global_config.anybox;
    let apiserver_config = global_config.apiserver.unwrap_or_default();
    info!("配置加载成功");

    // 启动图片清理任务
    image::start_cleanup_task(image_hosting_config.clone());

    // 注册自定义指标到 default registry
    if let Err(e) = image::register_metrics() {
        error!("注册自定义指标失败: {e}");
    }

    // 配置 Prometheus 指标采集（使用 default registry）
    let prometheus_layer = axum_prometheus::PrometheusMetricLayerBuilder::new()
        .with_default_metrics()
        .build_pair();
    let _metric_handle = prometheus_layer.1.clone();

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 前端静态文件目录
    let frontend_dir = "webserver/frontend/dist";
    let index_file = format!("{frontend_dir}/index.html");

    let has_frontend = Path::new(&frontend_dir).exists();

    let mut app = Router::new()
        .route("/metrics", axum::routing::get(metrics_handler))
        .nest(
            "/api/ocr",
            ocr::create_routes(remote_ocr_config, image_hosting_config.storage_dir.clone()),
        )
        .nest("/api/image", image::create_routes(image_hosting_config))
        .nest("/api/rc", rc::create_routes());

    // 添加 Anybox 路由（如果配置存在）
    if let Some(anybox_cfg) = anybox_config {
        info!("启用 Anybox 服务");
        let anybox_routes = anybox::create_routes(anybox_cfg).await?;
        app = app.nest("/api/anybox", anybox_routes);
    }

    if !has_frontend {
        error!("前端文件未找到: {frontend_dir}");
        error!("运行 'cd webserver/frontend && npm run build' 构建前端");
        panic!("前端文件未找到，服务器启动中止");
    } else {
        app = app
            .nest_service("/assets", ServeDir::new(format!("{frontend_dir}/assets")))
            .fallback_service(
                ServeDir::new(frontend_dir).not_found_service(ServeFile::new(&index_file)),
            );
        info!("前端服务已启用");
    }

    // 添加中间件
    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(prometheus_layer.0)
            .layer(cors),
    );

    // 监听地址
    let listen_address = apiserver_config.listen_address;
    let (host, port) = listen_address
        .split_once(':')
        .map(|(h, p)| (h.to_string(), p.parse::<u16>().unwrap_or(3000)))
        .unwrap_or(("127.0.0.1".to_string(), 3000));

    let addr = SocketAddr::from((
        host.parse::<IpAddr>()
            .unwrap_or("127.0.0.1".parse().unwrap()),
        port,
    ));
    info!(
        "服务器监听地址: {addr}, API 接口地址: http://{host}:{port}/api/, 前端页面地址: http://{host}:{port}/"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
