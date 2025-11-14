use axum::{Router, response::Json, routing::get};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower::limit::GlobalConcurrencyLimitLayer;

#[derive(Debug, Snafu)]
pub enum CustomHttpError {
    #[snafu(display("Internal Server 异常 (code: {}): {}", code, message))]
    InternalServerError { code: u16, message: String },
    #[snafu(display("Resource not found: {}", resource))]
    NotFound { resource: String },
    #[snafu(display("IO error: {}", source))]
    IoError { source: std::io::Error },
    /// 启动 HTTP 服务器失败
    #[snafu(display("Failed to start HTTP server: {}", source))]
    ServerStartupError { source: std::io::Error },
}

#[derive(Serialize, Deserialize)]
struct HealthCheckResponse {
    status: String,
    message: String,
}

async fn health_check() -> Json<HealthCheckResponse> {
    Json(HealthCheckResponse {
        status: "ok".to_string(),
        message: "Server is running".to_string(),
    })
}

pub struct HttpServer {
    addr: SocketAddr,
    config: ServerConfig,
}

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_concurrency: usize,
}

impl HttpServer {
    pub fn new(config: ServerConfig) -> Self {
        let host_ip = config.host.parse().unwrap_or_else(|_| {
            panic!("Invalid host IP address: {}", config.host);
        });
        let addr = SocketAddr::new(host_ip, config.port);

        Self { addr, config }
    }

    pub async fn run(self) -> Result<(), CustomHttpError> {
        // Build our application with routes
        let app =
            Router::new()
                .route("/health", get(health_check))
                .layer(
                    ServiceBuilder::new().layer(GlobalConcurrencyLimitLayer::new(
                        self.config.max_concurrency,
                    )),
                );

        // Create a TcpListener
        let listener = TcpListener::bind(self.addr).await.context(IoSnafu)?;
        println!("Listening on {}", self.addr);

        // Run the server
        axum::serve(listener, app)
            .await
            .context(ServerStartupSnafu)?;

        Ok(())
    }
}
