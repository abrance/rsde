use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use config::{
    GlobalConfig,
    apiserver::ApiServerConfig,
    datalink_engine::{DataLinkEngineBackend, DataLinkEngineConfig},
    image_host::ImageHostingConfig,
    nodemanage::NodeManageConfig,
    ocr::RemoteOcrConfig,
};
use serde_json::{Value, json};
use tower::ServiceExt;

fn build_config() -> GlobalConfig {
    GlobalConfig {
        apiserver: Some(ApiServerConfig::default()),
        remote_ocr: Some(RemoteOcrConfig {
            perm_url: "https://example.com/perm".to_string(),
            start_url: "https://example.com/start".to_string(),
            status_url: "https://example.com/status".to_string(),
            auth_token: "token".to_string(),
            auth_uuid: "uuid".to_string(),
            auth_cookie: "cookie".to_string(),
            origin: "https://example.com".to_string(),
            mode: "single".to_string(),
            timeout_secs: 30,
            poll_interval_ms: 500,
            poll_max_attempts: 20,
            poll_initial_delay_ms: 0,
            accept_invalid_certs: false,
        }),
        image_hosting: Some(ImageHostingConfig {
            storage_dir: "/tmp/rsde-test-images".to_string(),
            ..Default::default()
        }),
        datalink_engine: Some(DataLinkEngineConfig {
            backend: DataLinkEngineBackend::Memory,
            mysql: None,
        }),
        nodemanage: Some(NodeManageConfig::default()),
        ..Default::default()
    }
}

fn make_json_request(method: Method, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn build_app_bootstraps_heartbeat_datalink_for_nodemanage_refresh() {
    let config = build_config();
    let app = apiserver::build_app_for_test(config)
        .await
        .expect("build app");

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nodes/node",
            json!({
                "name": "worker-app",
                "endpoint": "http://worker-app:8080",
                "labels": ["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    let refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/nodes/node/{node_id}/status/refresh"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh.status(), StatusCode::OK);

    let loaded = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nodes/node/{node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(loaded.status(), StatusCode::OK);
    let loaded_json = read_json(loaded).await;
    assert_eq!(loaded_json["data"]["status"], "offline");
}

#[test]
fn build_datalink_router_keeps_existing_test_surface() {
    let router: Router = apiserver::build_datalink_v1_router(DataLinkEngineConfig {
        backend: DataLinkEngineBackend::Memory,
        mysql: None,
    })
    .expect("router");

    let _ = router;
}
