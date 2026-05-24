pub mod anybox;
pub mod datalink_engine;
pub mod image;
pub mod object_storage;
pub mod ocr;
pub mod prompt;

use axum::Router;
use config::datalink_engine::DataLinkEngineConfig;

pub fn build_datalink_v1_router(config: DataLinkEngineConfig) -> anyhow::Result<Router> {
    let routes = datalink_engine::create_routes(config)?;
    Ok(Router::new().nest("/api/datalink/v1", routes))
}
