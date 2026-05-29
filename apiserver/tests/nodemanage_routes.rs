use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use config::mysql::MysqlConfig;
use serde_json::{Value, json};
use std::env;
use tower::ServiceExt;

async fn build_shared_memory_app() -> (Router, apiserver::datalink_engine::SharedMemoryRuntime) {
    let shared = apiserver::datalink_engine::SharedMemoryRuntime::new();
    let datalink_routes = apiserver::datalink_engine::create_routes_with_shared_memory(
        config::datalink_engine::DataLinkEngineConfig {
            backend: config::datalink_engine::DataLinkEngineBackend::Memory,
            mysql: None,
        },
        shared.clone(),
    )
    .expect("build datalink routes");
    let nodemanage_routes = apiserver::nodemanage::create_routes_with_shared_memory(
        config::nodemanage::NodeManageConfig::default(),
        shared.clone(),
    )
    .await
    .expect("build nodemanage routes");

    (
        Router::new()
            .nest("/api/datalink/v1", datalink_routes)
            .nest("/api/nodes", nodemanage_routes),
        shared,
    )
}

fn bootstrapped_heartbeat_apply_payload(result_table_name: &str) -> Value {
    json!({
        "name": "nodemanage_node_heartbeat",
        "description": "shared heartbeat datalink for all managed nodes",
        "domain": "nodemanage",
        "owner_service": "nodemanage",
        "data_type": "metric",
        "status": "active",
        "status_message": null,
        "datasource": {
            "producer": "rsagent",
            "data_type": "metric",
            "collect_method": "agent",
            "protocol": "http",
            "interval_seconds": 60,
            "labels": {
                "domain": "nodemanage",
                "link_purpose": "node_heartbeat"
            },
            "dimension_keys": ["node_id", "agent_id", "node_ip"],
            "auth_ref": null,
            "config": {}
        },
        "etl_pipeline": {
            "mode": "passthrough",
            "config": {}
        },
        "result_table": {
            "result_table_name": result_table_name,
            "storage_type": "victoriametrics",
            "storage_cluster": "default",
            "database": null,
            "table_name": null,
            "metric_name": "nm_node_heartbeat",
            "schema": {
                "timestamp": "datetime",
                "node_id": "string",
                "agent_id": "string",
                "node_ip": "string"
            },
            "retention_days": 7,
            "query_template": "query heartbeat"
        }
    })
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

fn mysql_test_config() -> MysqlConfig {
    MysqlConfig {
        host: env::var("MYSQL_HOST")
            .unwrap_or_else(|_| "test-mysql.bkbase-test.svc.cluster.local".to_string()),
        port: env::var("MYSQL_PORT")
            .unwrap_or_else(|_| "3306".to_string())
            .parse()
            .unwrap_or(3306),
        user: env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
        password: env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "testpass".to_string()),
        database: env::var("MYSQL_DATABASE").unwrap_or_else(|_| "prompt".to_string()),
        max_connections: 10,
        min_connections: 1,
        connect_timeout_secs: 10,
    }
}

fn mysql_backed_nodemanage_config(table_prefix: String) -> config::nodemanage::NodeManageConfig {
    config::nodemanage::NodeManageConfig {
        table_prefix,
        mysql: Some(mysql_test_config()),
        ..Default::default()
    }
}

#[test]
fn nodemanage_assembly_applies_config_install_defaults_without_silent_fallback() {
    let config = config::nodemanage::NodeManageConfig {
        table_prefix: "node_contract_".to_string(),
        mysql: Some(mysql_test_config()),
        rsagent_package_url: Some("https://example.com/from-config.tar.gz".to_string()),
        install_root: "/srv/rsagent".to_string(),
        register_callback_url: "http://10.0.0.1:3000/api/nodes/agent/register".to_string(),
        install_plugins: vec![config::nodemanage::InstallPluginConfig {
            name: "metrics".to_string(),
            version: "1.2.3".to_string(),
            package_url: Some("https://example.com/plugins/metrics.tar.gz".to_string()),
        }],
        register_wait_timeout_secs: 45,
        ssh_connect_timeout_secs: 12,
        heartbeat: config::nodemanage::HeartbeatDataLinkConfig::default(),
    };

    let request = nodemanage::InstallNodeRequest {
        host: "10.0.0.8".to_string(),
        ssh_port: 22,
        username: "root".to_string(),
        password: Some("secret".to_string()),
        private_key: None,
        rsagent_package_url: String::new(),
        install_root: String::new(),
        register_callback_url: String::new(),
        plugins: vec![],
        labels: vec!["edge".to_string()],
    };

    let resolved = apiserver::nodemanage::apply_install_request_defaults(&config, request);

    assert_eq!(
        resolved.rsagent_package_url,
        "https://example.com/from-config.tar.gz"
    );
    assert_eq!(resolved.install_root, "/srv/rsagent");
    assert_eq!(
        resolved.register_callback_url,
        "http://10.0.0.1:3000/api/nodes/agent/register"
    );
    assert_eq!(resolved.plugins.len(), 1);
    assert_eq!(resolved.plugins[0].name, "metrics");
    assert_eq!(resolved.plugins[0].version, "1.2.3");
    assert_eq!(
        resolved.plugins[0].package_url.as_deref(),
        Some("https://example.com/plugins/metrics.tar.gz")
    );
}

#[tokio::test]
async fn nodemanage_routes_support_health_create_list_and_install() {
    let app = apiserver::nodemanage::create_routes(config::nodemanage::NodeManageConfig::default())
        .await
        .unwrap();

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/node")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"worker-1","endpoint":"http://worker-1:8080","labels":["gpu"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);

    let list = app
        .clone()
        .oneshot(Request::builder().uri("/node").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);

    let install = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/install")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"host":"10.0.0.8","ssh_port":22,"username":"root","password":"secret","rsagent_package_url":"https://example.com/rsagent.tar.gz","install_root":"/opt/rsagent","register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/register","plugins":[],"labels":["edge"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::OK);
}

#[tokio::test]
async fn nodemanage_install_uses_config_package_url_when_request_omits_it() {
    let app = apiserver::nodemanage::create_routes(config::nodemanage::NodeManageConfig {
        rsagent_package_url: Some("https://example.com/from-config.tar.gz".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();

    let install = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/install")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"host":"10.0.0.8","ssh_port":22,"username":"root","password":"secret","rsagent_package_url":"","install_root":"/opt/rsagent","register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/register","plugins":[],"labels":["edge"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(install.status(), StatusCode::OK);
    let body = axum::body::to_bytes(install.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn nodemanage_refresh_status_uses_shared_datalink_runtime() {
    let (app, _shared) = build_shared_memory_app().await;

    let bootstrapped = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks/by-result-table/nm_node_heartbeat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrapped.status(), StatusCode::OK);
    let bootstrapped_json = read_json(bootstrapped).await;
    assert_eq!(
        bootstrapped_json["data"]["result_table"]["result_table_name"],
        "nm_node_heartbeat"
    );

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nodes/node",
            json!({
                "name": "worker-refresh",
                "endpoint": "http://worker-refresh:8080",
                "labels": ["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    let marked_online = app
        .clone()
        .oneshot(make_json_request(
            Method::PATCH,
            &format!("/api/nodes/node/{node_id}/status"),
            json!({"status": "online"}),
        ))
        .await
        .unwrap();
    assert_eq!(marked_online.status(), StatusCode::OK);

    let refreshed = app
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
    assert_eq!(refreshed.status(), StatusCode::OK);
    let refreshed_json = read_json(refreshed).await;
    assert_eq!(refreshed_json["success"], true);
    assert_eq!(refreshed_json["data"]["status"], "offline");
    assert!(refreshed_json["data"]["last_heartbeat_at"].is_null());

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

#[tokio::test]
async fn nodemanage_refresh_status_tracks_bootstrapped_datalink_id_after_table_rename() {
    let (app, shared) = build_shared_memory_app().await;

    let bootstrapped = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks/by-result-table/nm_node_heartbeat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrapped.status(), StatusCode::OK);
    let bootstrapped_json = read_json(bootstrapped).await;
    let data_link_id = bootstrapped_json["data"]["data_link"]["data_link_id"]
        .as_str()
        .unwrap()
        .to_string();

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/datalink/v1/datalinks:apply")
                .header("content-type", "application/json")
                .header(
                    "x-idempotency-key",
                    "nodemanage-bootstrap-heartbeat-datalink-v2",
                )
                .body(Body::from(
                    bootstrapped_heartbeat_apply_payload("nm_node_heartbeat_v2").to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_json = read_json(updated).await;
    assert_eq!(
        updated_json["data"]["data_link"]["data_link_id"],
        data_link_id
    );
    assert_eq!(
        updated_json["data"]["result_table"]["result_table_name"],
        "nm_node_heartbeat_v2"
    );

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nodes/node",
            json!({
                "name": "worker-refresh-rename",
                "endpoint": "http://worker-refresh-rename:8080",
                "labels": ["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    shared.heartbeat_store.insert(
        "nm_node_heartbeat_v2",
        query_engine::HeartbeatSample {
            node_id: node_id.clone(),
            observed_at: chrono::Utc::now(),
        },
    );

    let refreshed = app
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
    assert_eq!(refreshed.status(), StatusCode::OK);
    let refreshed_json = read_json(refreshed).await;
    assert_eq!(refreshed_json["data"]["status"], "online");
    assert!(refreshed_json["data"]["last_heartbeat_at"].is_string());
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn nodemanage_register_persists_through_configured_repository_path() {
    let table_prefix = format!("node_route_test_{}_", uuid::Uuid::new_v4().simple());
    let config = mysql_backed_nodemanage_config(table_prefix.clone());
    let app = apiserver::nodemanage::create_routes(config.clone())
        .await
        .unwrap();

    let register = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/agent/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"agent_id":"agent-route-1","hostname":"worker-route","endpoint":"http://worker-route:19090","labels":["edge"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register.status(), StatusCode::OK);

    let rebuilt_app = apiserver::nodemanage::create_routes(config).await.unwrap();

    let list = rebuilt_app
        .oneshot(Request::builder().uri("/node").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);

    let body = axum::body::to_bytes(list.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["total"], 1);
    assert_eq!(json["data"]["items"][0]["id"], "agent-route-1");
}
