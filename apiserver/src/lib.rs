pub mod anybox;
pub mod datalink_engine;
pub mod image;
pub mod nodemanage;
pub mod object_storage;
pub mod ocr;
pub mod prompt;

use axum::Router;
use config::datalink_engine::DataLinkEngineConfig;
use tower_http::services::{ServeDir, ServeFile};

pub fn build_datalink_v1_router(config: DataLinkEngineConfig) -> anyhow::Result<Router> {
    let routes = datalink_engine::create_routes(config)?;
    Ok(Router::new().nest("/api/datalink/v1", routes))
}

pub fn build_frontend_router(frontend_dir: &str) -> Router {
    let index_file = format!("{frontend_dir}/index.html");

    Router::new()
        .nest_service("/assets", ServeDir::new(format!("{frontend_dir}/assets")))
        .fallback_service(ServeFile::new(index_file))
}
