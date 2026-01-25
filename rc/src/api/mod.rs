mod kafka;
mod mysql;
mod redis;

use axum::Router;

/// 创建 RC 服务的所有路由
pub fn create_routes() -> Router {
    Router::new()
        .nest("/kafka", kafka::create_routes())
        .nest("/redis", redis::create_routes())
        .nest("/mysql", mysql::create_routes())
}
