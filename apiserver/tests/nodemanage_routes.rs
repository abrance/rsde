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
            .nest("/api/nodes", nodemanage_routes.clone())
            .nest(
                "/api/nm/v1",
                apiserver::nodemanage::create_v1_routes_with_shared_memory(
                    config::nodemanage::NodeManageConfig::default(),
                    shared.clone(),
                )
                .await
                .expect("build nodemanage v1 routes"),
            ),
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

fn agent_sync_request(node_id: Option<&str>) -> Value {
    json!({
        "agent_id": "agent-route-1",
        "node_id": node_id,
        "agent_version": "0.1.0",
        "hostname": "worker-route",
        "os_family": "linux",
        "os_distribution": "ubuntu",
        "arch": "x86_64",
        "capabilities": ["script", "command"],
        "started_at": "2026-05-29T10:00:00Z",
        "config_version": null
    })
}

fn create_node_request(name: &str, endpoint: &str, labels: &[&str]) -> Value {
    json!({
        "name": name,
        "endpoint": endpoint,
        "labels": labels,
    })
}

fn rebind_request(target_agent_id: &str, reason: Option<&str>) -> Value {
    json!({
        "target_agent_id": target_agent_id,
        "reason": reason,
    })
}

#[test]
fn nodemanage_assembly_applies_config_install_defaults_without_silent_fallback() {
    let config = config::nodemanage::NodeManageConfig {
        table_prefix: "node_contract_".to_string(),
        mysql: Some(mysql_test_config()),
        rsagent_package_url: Some("https://example.com/from-config.tar.gz".to_string()),
        install_root: "/srv/rsagent".to_string(),
        register_callback_url: "http://10.0.0.1:3000/api/nodes/agent/sync".to_string(),
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
        "http://10.0.0.1:3000/api/nodes/agent/sync"
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
                    r#"{"host":"10.0.0.8","ssh_port":22,"username":"root","password":"secret","rsagent_package_url":"https://example.com/rsagent.tar.gz","install_root":"/opt/rsagent","register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/sync","plugins":[],"labels":["edge"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::OK);
}

#[tokio::test]
async fn nodemanage_v1_list_nodes_returns_node_summary_page() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-v1", "http://worker-v1:8080", &["environment:test"]),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);

    let listed = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/nm/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let body = read_json(listed).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["items"][0]["environment"], "test");
    assert_eq!(body["data"]["items"][0]["install_phase"], "not_started");
}

#[tokio::test]
async fn nodemanage_v1_get_node_detail_returns_aggregate() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-detail", "http://worker-detail:8080", &["env:prod"]),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let detail = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let body = read_json(detail).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["node"]["node_id"], node_id);
    assert_eq!(body["data"]["node"]["environment"], "prod");
    assert_eq!(body["data"]["status"]["install_phase"], "not_started");
}

#[tokio::test]
async fn nodemanage_v1_get_node_detail_exposes_desensitized_latest_install_task() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request(
                "worker-detail-install",
                "http://worker-detail-install:8080",
                &["env:prod"],
            ),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let install = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            &format!("/api/nm/v1/nodes/{node_id}/install"),
            json!({
                "host":"10.0.0.18",
                "ssh_port":22,
                "username":"root",
                "password":"secret",
                "rsagent_package_url":"https://example.com/rsagent.tar.gz",
                "install_root":"/opt/rsagent",
                "register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/sync",
                "plugins":[],
                "labels":["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::ACCEPTED);

    let detail = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let body = read_json(detail).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["latest_install_task"]["node_id"], node_id);
    assert_eq!(
        body["data"]["latest_install_task"]["request_summary"]["host"],
        "10.0.0.18"
    );
    assert_eq!(
        body["data"]["latest_install_task"]["request_summary"]["username"],
        "root"
    );
    assert_eq!(
        body["data"]["latest_install_task"]["request_summary"]["labels"],
        json!(["edge"])
    );
    assert!(
        body["data"]["latest_install_task"]
            .get("request_host")
            .is_none()
    );
    assert!(!body.to_string().contains("password"));
    assert!(!body.to_string().contains("private_key"));
    assert!(!body.to_string().contains("register_callback_url"));
}

#[tokio::test]
async fn nodemanage_v1_get_status_batch_returns_projection() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-batch", "http://worker-batch:8080", &[]),
        ))
        .await
        .unwrap();
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let batch = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/status:batch?node_ids={node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(batch.status(), StatusCode::OK);
    let body = read_json(batch).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"][0]["node_id"], node_id);
}

#[tokio::test]
async fn nodemanage_v1_install_returns_accepted_task_handle() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-install", "http://worker-install:8080", &[]),
        ))
        .await
        .unwrap();
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let install = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            &format!("/api/nm/v1/nodes/{node_id}/install"),
            json!({
                "host":"10.0.0.8",
                "ssh_port":22,
                "username":"root",
                "password":"secret",
                "rsagent_package_url":"https://example.com/rsagent.tar.gz",
                "install_root":"/opt/rsagent",
                "register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/sync",
                "plugins":[],
                "labels":["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::ACCEPTED);
    let body = read_json(install).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["node_id"], node_id);
    assert_eq!(body["data"]["accepted"], true);
    assert!(body["data"]["install_task_id"].is_string());

    let install_task_id = body["data"]["install_task_id"].as_str().unwrap();

    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/install-tasks/{install_task_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task.status(), StatusCode::OK);
    let task_body = read_json(task).await;
    assert_eq!(task_body["success"], true);
    assert_eq!(task_body["data"]["node_id"], node_id);
    assert_eq!(task_body["data"]["task_state"], "pending");
    assert_eq!(task_body["data"]["request_summary"]["host"], "10.0.0.8");
    assert_eq!(task_body["data"]["request_summary"]["ssh_port"], 22);
    assert_eq!(task_body["data"]["request_summary"]["username"], "root");
    assert_eq!(
        task_body["data"]["request_summary"]["labels"],
        json!(["edge"])
    );
    assert!(task_body["data"].get("request_host").is_none());
    assert!(task_body.to_string().contains("request_summary"));
    assert!(!task_body.to_string().contains("password"));
    assert!(!task_body.to_string().contains("private_key"));

    let latest = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}/install-tasks:latest"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(latest.status(), StatusCode::OK);
    let latest_body = read_json(latest).await;
    assert_eq!(latest_body["success"], true);
    assert_eq!(latest_body["data"]["node_id"], node_id);
    assert_eq!(latest_body["data"]["request_summary"]["host"], "10.0.0.8");
    assert!(latest_body["data"].get("request_host").is_none());
}

#[tokio::test]
async fn nodemanage_v1_latest_install_task_returns_not_found_for_missing_node() {
    let (app, _) = build_shared_memory_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/nm/v1/nodes/missing-node/install-tasks:latest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = read_json(response).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "NODE_NOT_FOUND");
    assert!(body["error"]["details"].is_null());
}

#[tokio::test]
async fn nodemanage_v1_binding_returns_not_found_for_missing_binding() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-no-binding", "http://worker-no-binding:8080", &[]),
        ))
        .await
        .unwrap();
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}/binding"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "BINDING_NOT_FOUND");
    assert!(body["error"]["details"].is_null());
}

#[tokio::test]
async fn nodemanage_v1_rebind_promotes_target_binding_and_updates_read_models() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request("worker-rebind", "http://worker-rebind:8080", &["env:test"]),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    let old_sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/agents/sync",
            json!({
                "agent_id": "agent-old",
                "node_id": node_id,
                "agent_version": "0.1.0",
                "hostname": "worker-old",
                "os_family": "linux",
                "os_distribution": "ubuntu",
                "arch": "x86_64",
                "capabilities": ["script", "command"],
                "started_at": "2026-05-29T10:00:00Z",
                "config_version": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(old_sync.status(), StatusCode::OK);

    let target_sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/agents/sync",
            json!({
                "agent_id": "agent-old",
                "node_id": node_id,
                "agent_version": "0.1.0",
                "hostname": "worker-new",
                "os_family": "linux",
                "os_distribution": "ubuntu",
                "arch": "x86_64",
                "capabilities": ["script", "command"],
                "started_at": "2026-05-29T10:00:00Z",
                "config_version": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(target_sync.status(), StatusCode::OK);

    let rebind = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            &format!("/api/nm/v1/nodes/{node_id}/rebind"),
            rebind_request("agent-old", Some("manual_rebind_after_conflict")),
        ))
        .await
        .unwrap();
    assert_eq!(rebind.status(), StatusCode::OK);
    let rebind_body = read_json(rebind).await;
    assert_eq!(rebind_body["success"], true);
    assert_eq!(rebind_body["data"]["accepted"], true);
    assert_eq!(rebind_body["data"]["node_id"], node_id);
    assert_eq!(rebind_body["data"]["target_agent_id"], "agent-old");
    assert_eq!(rebind_body["data"]["binding_state"], "bound");
    assert!(rebind_body["data"]["previous_agent_id"].is_null());

    let binding = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}/binding"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(binding.status(), StatusCode::OK);
    let binding_body = read_json(binding).await;
    assert_eq!(binding_body["data"]["agent_id"], "agent-old");
    assert_eq!(binding_body["data"]["binding_state"], "bound");

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/{node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_body = read_json(detail).await;
    assert_eq!(detail_body["data"]["binding"]["agent_id"], "agent-old");
    assert_eq!(detail_body["data"]["status"]["binding_state"], "bound");

    let batch = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/nm/v1/nodes/status:batch?node_ids={node_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(batch.status(), StatusCode::OK);
    let batch_body = read_json(batch).await;
    assert_eq!(batch_body["data"][0]["binding_state"], "bound");
}

#[tokio::test]
async fn nodemanage_v1_rebind_returns_not_found_for_unknown_target_agent() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request(
                "worker-rebind-missing-target",
                "http://worker-rebind-missing-target:8080",
                &[],
            ),
        ))
        .await
        .unwrap();
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap();

    let response = app
        .oneshot(make_json_request(
            Method::POST,
            &format!("/api/nm/v1/nodes/{node_id}/rebind"),
            rebind_request("agent-missing", None),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "TARGET_AGENT_NOT_FOUND");
    assert!(body["error"]["details"].is_null());
}

#[tokio::test]
async fn nodemanage_v1_rebind_returns_conflict_when_target_agent_bound_elsewhere() {
    let (app, _) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/nodes",
            create_node_request(
                "worker-rebind-conflict",
                "http://worker-rebind-conflict:8080",
                &[],
            ),
        ))
        .await
        .unwrap();
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    let old_sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/agents/sync",
            json!({
                "agent_id": "agent-old",
                "node_id": node_id,
                "agent_version": "0.1.0",
                "hostname": "worker-old",
                "os_family": "linux",
                "os_distribution": "ubuntu",
                "arch": "x86_64",
                "capabilities": ["script", "command"],
                "started_at": "2026-05-29T10:00:00Z",
                "config_version": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(old_sync.status(), StatusCode::OK);

    let target_sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nm/v1/agents/sync",
            json!({
                "agent_id": "agent-target",
                "node_id": "other-node",
                "agent_version": "0.1.0",
                "hostname": "worker-target",
                "os_family": "linux",
                "os_distribution": "ubuntu",
                "arch": "x86_64",
                "capabilities": ["script", "command"],
                "started_at": "2026-05-29T10:00:00Z",
                "config_version": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(target_sync.status(), StatusCode::OK);

    let response = app
        .oneshot(make_json_request(
            Method::POST,
            &format!("/api/nm/v1/nodes/{node_id}/rebind"),
            rebind_request("agent-target", Some("manual_rebind_after_conflict")),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "REBIND_TARGET_ALREADY_BOUND");
    assert!(body["error"]["details"].is_null());
}

#[tokio::test]
async fn nodemanage_v1_install_task_returns_not_found_for_missing_task() {
    let (app, _) = build_shared_memory_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/nm/v1/install-tasks/missing-task")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["error"]["code"], "INSTALL_TASK_NOT_FOUND");
    assert!(body["error"]["details"].is_null());
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
                    r#"{"host":"10.0.0.8","ssh_port":22,"username":"root","password":"secret","rsagent_package_url":"","install_root":"/opt/rsagent","register_callback_url":"http://127.0.0.1:3000/api/nodes/agent/sync","plugins":[],"labels":["edge"]}"#,
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
async fn nodemanage_sync_returns_structured_desired_state() {
    let app = apiserver::nodemanage::create_routes(config::nodemanage::NodeManageConfig::default())
        .await
        .unwrap();

    let response = app
        .oneshot(make_json_request(
            Method::POST,
            "/agent/sync",
            agent_sync_request(Some("node-route-1")),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["error"].is_null());
    assert_eq!(body["data"]["accepted"], true);
    assert_eq!(body["data"]["agent_id"], "agent-route-1");
    assert_eq!(body["data"]["bound_node_id"], "node-route-1");
    assert_eq!(body["data"]["binding_state"], "bound");
    assert_eq!(body["data"]["agent_run_mode"], "active");
    assert_eq!(body["data"]["config_version"], "2026-05-29T10:00:00Z");
    assert_eq!(body["data"]["heartbeat_config"]["version"], "1");
    assert_eq!(
        body["data"]["heartbeat_config"]["data_link_id"],
        "dl_heartbeat_001"
    );
    assert_eq!(
        body["data"]["heartbeat_config"]["vm_base_url"],
        "http://victoriametrics:8428"
    );
    assert_eq!(body["data"]["heartbeat_config"]["interval_secs"], 60);
    assert_eq!(body["data"]["job_manage_config"]["version"], "1");
    assert_eq!(
        body["data"]["job_manage_config"]["base_url"],
        "http://job-manage:3000/api/job-manage/v1/tasks"
    );
    assert_eq!(
        body["data"]["job_manage_config"]["task_filter_defaults"]["states"],
        json!(["queued", "acknowledged", "running"])
    );
    assert_eq!(body["data"]["sync_interval_secs"], 30);
    assert_eq!(body["data"]["task_sync_interval_secs"], 10);
}

#[tokio::test]
async fn nodemanage_sync_heartbeat_config_resolves_bootstrapped_query_path() {
    let (app, shared) = build_shared_memory_app().await;

    let created = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nodes/node",
            json!({
                "name": "worker-sync-runtime",
                "endpoint": "http://worker-sync-runtime:8080",
                "labels": ["edge"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_json = read_json(created).await;
    let node_id = created_json["data"]["id"].as_str().unwrap().to_string();

    let sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/nodes/agent/sync",
            agent_sync_request(Some(&node_id)),
        ))
        .await
        .unwrap();
    assert_eq!(sync.status(), StatusCode::OK);
    let sync_json = read_json(sync).await;
    let heartbeat_data_link_id = sync_json["data"]["heartbeat_config"]["data_link_id"]
        .as_str()
        .unwrap()
        .to_string();

    let resolved = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/api/datalink/v1/datalinks/{heartbeat_data_link_id}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resolved.status(), StatusCode::OK);
    let resolved_json = read_json(resolved).await;
    let result_table_name = resolved_json["data"]["result_table"]["result_table_name"]
        .as_str()
        .unwrap()
        .to_string();

    shared.heartbeat_store.insert(
        &result_table_name,
        query_engine::HeartbeatSample {
            node_id: node_id.clone(),
            observed_at: chrono::Utc::now(),
        },
    );

    let refreshed = app
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
async fn nodemanage_sync_returns_explicit_unbound_rejection() {
    let app = apiserver::nodemanage::create_routes(config::nodemanage::NodeManageConfig::default())
        .await
        .unwrap();

    let response = app
        .oneshot(make_json_request(
            Method::POST,
            "/agent/sync",
            agent_sync_request(None),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert!(body["error"].is_null());
    assert_eq!(body["data"]["accepted"], false);
    assert_eq!(body["data"]["agent_id"], "agent-route-1");
    assert_eq!(body["data"]["bound_node_id"], "");
    assert_eq!(body["data"]["binding_state"], "unbound");
    assert_eq!(body["data"]["agent_run_mode"], "idle");
    assert_eq!(
        body["data"]["rejection_reason"],
        "node_id is required for initial sync"
    );
}

#[tokio::test]
async fn nodemanage_sync_route_replaces_legacy_register_route() {
    let app = apiserver::nodemanage::create_routes(config::nodemanage::NodeManageConfig::default())
        .await
        .unwrap();

    let response = app
        .oneshot(make_json_request(
            Method::POST,
            "/agent/register",
            json!({
                "agent_id": "agent-route-1",
                "hostname": "worker-route",
                "endpoint": "http://worker-route:19090",
                "labels": ["edge"]
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn nodemanage_sync_persists_binding_through_configured_repository_path() {
    let table_prefix = format!("node_route_test_{}_", uuid::Uuid::new_v4().simple());
    let config = mysql_backed_nodemanage_config(table_prefix.clone());
    let app = apiserver::nodemanage::create_routes(config.clone())
        .await
        .unwrap();

    let initial_sync = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/agent/sync",
            agent_sync_request(Some("node-route-1")),
        ))
        .await
        .unwrap();
    assert_eq!(initial_sync.status(), StatusCode::OK);
    let initial_body = read_json(initial_sync).await;
    assert_eq!(initial_body["success"], true);
    assert_eq!(initial_body["data"]["accepted"], true);
    assert_eq!(initial_body["data"]["bound_node_id"], "node-route-1");

    let rebuilt_app = apiserver::nodemanage::create_routes(config).await.unwrap();

    let follow_up_sync = rebuilt_app
        .oneshot(make_json_request(
            Method::POST,
            "/agent/sync",
            agent_sync_request(None),
        ))
        .await
        .unwrap();
    assert_eq!(follow_up_sync.status(), StatusCode::OK);

    let body = read_json(follow_up_sync).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["accepted"], true);
    assert_eq!(body["data"]["agent_id"], "agent-route-1");
    assert_eq!(body["data"]["bound_node_id"], "node-route-1");
    assert_eq!(body["data"]["binding_state"], "bound");
}
