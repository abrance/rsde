pub mod anybox;
pub mod datalink_engine;
pub mod image;
pub mod nodemanage;
pub mod object_storage;
pub mod ocr;
pub mod prompt;

use axum::Router;
use config::{
    GlobalConfig,
    datalink_engine::{DataLinkEngineBackend, DataLinkEngineConfig},
};
use tower_http::services::{ServeDir, ServeFile};

pub fn build_datalink_v1_router(config: DataLinkEngineConfig) -> anyhow::Result<Router> {
    let routes = datalink_engine::create_routes(config)?;
    Ok(Router::new().nest("/api/datalink/v1", routes))
}

pub async fn build_api_app(global_config: GlobalConfig) -> anyhow::Result<Router> {
    let remote_ocr_config = global_config
        .remote_ocr
        .ok_or_else(|| anyhow::anyhow!("配置文件中缺少 [remote_ocr] 部分"))?;
    let image_hosting_config = global_config
        .image_hosting
        .ok_or_else(|| anyhow::anyhow!("配置文件中缺少 [image_hosting] 部分"))?;
    let anybox_config = global_config.anybox;
    let prompt_config = global_config.prompt;
    let object_storage_config = global_config.object_storage;
    let datalink_engine_config = global_config.datalink_engine;
    let nodemanage_config = global_config.nodemanage;

    let mut app = Router::new()
        .nest(
            "/api/ocr",
            ocr::create_routes(remote_ocr_config, image_hosting_config.storage_dir.clone()),
        )
        .nest("/api/image", image::create_routes(image_hosting_config))
        .nest("/api/rc", rc::create_routes());

    if let Some(anybox_cfg) = anybox_config {
        let anybox_routes = anybox::create_routes(anybox_cfg).await?;
        app = app.nest("/api/anybox", anybox_routes);
    }

    if let Some(prompt_cfg) = prompt_config {
        let prompt_routes = prompt::create_routes(prompt_cfg).await?;
        app = app.nest("/api/prompt", prompt_routes);
    }

    if let Some(object_storage_cfg) = object_storage_config {
        let object_storage_routes = object_storage::create_routes(object_storage_cfg);
        app = app.nest("/api/object-storage", object_storage_routes);
    }

    match (datalink_engine_config, nodemanage_config) {
        (Some(datalink_cfg), Some(nodemanage_cfg))
            if should_share_memory_runtime(&datalink_cfg, &nodemanage_cfg) =>
        {
            let shared = datalink_engine::SharedMemoryRuntime::new();
            let datalink_routes =
                datalink_engine::create_routes_with_shared_memory(datalink_cfg, shared.clone())?;
            let nodemanage_routes =
                nodemanage::create_routes_with_shared_memory(nodemanage_cfg, shared).await?;
            app = app
                .nest("/api/datalink/v1", datalink_routes)
                .nest("/api/nodes", nodemanage_routes);
        }
        (Some(datalink_cfg), maybe_nodemanage_cfg) => {
            let datalink_routes = datalink_engine::create_routes(datalink_cfg)?;
            app = app.nest("/api/datalink/v1", datalink_routes);

            if let Some(nodemanage_cfg) = maybe_nodemanage_cfg {
                let nodemanage_routes = nodemanage::create_routes(nodemanage_cfg).await?;
                app = app.nest("/api/nodes", nodemanage_routes);
            }
        }
        (None, Some(nodemanage_cfg)) => {
            let nodemanage_routes = nodemanage::create_routes(nodemanage_cfg).await?;
            app = app.nest("/api/nodes", nodemanage_routes);
        }
        (None, None) => {}
    }

    Ok(app)
}

pub async fn build_app_for_test(global_config: GlobalConfig) -> anyhow::Result<Router> {
    build_api_app(global_config).await
}

fn should_share_memory_runtime(
    datalink_cfg: &DataLinkEngineConfig,
    nodemanage_cfg: &config::nodemanage::NodeManageConfig,
) -> bool {
    matches!(&datalink_cfg.backend, DataLinkEngineBackend::Memory) && nodemanage_cfg.mysql.is_none()
}

pub fn build_frontend_router(frontend_dir: &str) -> Router {
    let index_file = format!("{frontend_dir}/index.html");

    Router::new()
        .nest_service("/assets", ServeDir::new(format!("{frontend_dir}/assets")))
        .fallback_service(ServeFile::new(index_file))
}
