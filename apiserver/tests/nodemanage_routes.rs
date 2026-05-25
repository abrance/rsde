use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use config::mysql::MysqlConfig;
use serde_json::Value;
use tower::ServiceExt;

fn mysql_backed_nodemanage_config(table_prefix: String) -> config::nodemanage::NodeManageConfig {
    config::nodemanage::NodeManageConfig {
        table_prefix,
        mysql: Some(MysqlConfig {
            host: "test-mysql.bkbase-test.svc.cluster.local".to_string(),
            port: 3306,
            user: "root".to_string(),
            password: "testpass".to_string(),
            database: "prompt".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_secs: 10,
        }),
        ..Default::default()
    }
}

#[test]
fn nodemanage_assembly_applies_config_install_defaults_without_silent_fallback() {
    let config = config::nodemanage::NodeManageConfig {
        table_prefix: "node_contract_".to_string(),
        mysql: Some(MysqlConfig {
            host: "test-mysql.bkbase-test.svc.cluster.local".to_string(),
            port: 3306,
            user: "root".to_string(),
            password: "testpass".to_string(),
            database: "prompt".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_secs: 10,
        }),
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
