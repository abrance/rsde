use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use config::{
    GlobalConfig,
    datalink_engine::{DataLinkEngineBackend, DataLinkEngineConfig},
};
use serde_json::{Value, json};
use tower::util::ServiceExt;

fn build_app() -> Router {
    apiserver::build_datalink_v1_router(DataLinkEngineConfig {
        backend: DataLinkEngineBackend::Memory,
        mysql: None,
    })
    .expect("build datalink v1 router")
}

fn apply_payload(
    name: &str,
    domain: &str,
    owner_service: &str,
    data_type: &str,
    status: &str,
    result_table_name: &str,
) -> Value {
    json!({
        "name": name,
        "description": format!("desc-{name}"),
        "domain": domain,
        "owner_service": owner_service,
        "data_type": data_type,
        "status": status,
        "status_message": null,
        "datasource": {
            "producer": format!("producer-{name}"),
            "data_type": data_type,
            "collect_method": "pull",
            "protocol": "http_poll",
            "interval_seconds": 60,
            "labels": {
                "domain": domain
            },
            "dimension_keys": ["tenant_id"],
            "auth_ref": "secret://token",
            "config": {
                "endpoint": format!("https://api.example.com/{name}")
            }
        },
        "etl_pipeline": {
            "mode": "vector",
            "config": {
                "mapping": "a:b"
            }
        },
        "result_table": {
            "result_table_name": result_table_name,
            "storage_type": "mysql",
            "storage_cluster": "cluster-a",
            "database": "analytics",
            "table_name": result_table_name,
            "schema": {
                "id": "string"
            },
            "retention_days": 7
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

#[tokio::test]
async fn health_endpoint_exists_for_datalink_routes() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn put_apply_returns_success_envelope() {
    let app = build_app();
    let response = app
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "apply-success",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_apply_success",
            ),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], Value::Bool(true));
    assert!(body["data"]["data_link"]["data_link_id"].is_string());
}

#[tokio::test]
async fn repeated_apply_with_same_logical_identity_is_idempotent() {
    let app = build_app();

    let first = app
        .clone()
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "idem-logical",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_idem_logical",
            ),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_json = read_json(first).await;
    let first_id = first_json["data"]["data_link"]["data_link_id"].clone();

    let second = app
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "idem-logical",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_idem_logical",
            ),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_json = read_json(second).await;
    let second_id = second_json["data"]["data_link"]["data_link_id"].clone();

    assert_eq!(first_id, second_id);
}

#[tokio::test]
async fn repeated_apply_with_same_idempotency_key_and_different_payload_returns_conflict() {
    let app = build_app();

    let first = Request::builder()
        .method(Method::PUT)
        .uri("/api/datalink/v1/datalinks:apply")
        .header("content-type", "application/json")
        .header("x-idempotency-key", "same-key")
        .body(Body::from(
            apply_payload(
                "idem-key-a",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_idem_key_a",
            )
            .to_string(),
        ))
        .unwrap();
    let first_resp = app.clone().oneshot(first).await.unwrap();
    assert_eq!(first_resp.status(), StatusCode::OK);

    let second = Request::builder()
        .method(Method::PUT)
        .uri("/api/datalink/v1/datalinks:apply")
        .header("content-type", "application/json")
        .header("x-idempotency-key", "same-key")
        .body(Body::from(
            apply_payload(
                "idem-key-b",
                "security",
                "svc-b",
                "event",
                "draft",
                "rt_idem_key_b",
            )
            .to_string(),
        ))
        .unwrap();
    let second_resp = app.oneshot(second).await.unwrap();
    assert_eq!(second_resp.status(), StatusCode::CONFLICT);
    let second_json = read_json(second_resp).await;

    assert_eq!(second_json["success"], Value::Bool(false));
    assert_eq!(
        second_json["error"]["code"],
        Value::String("DL_IDEMPOTENCY_CONFLICT".to_string())
    );
}

#[tokio::test]
async fn get_by_id_returns_full_bundle() {
    let app = build_app();

    let apply_resp = app
        .clone()
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "get-by-id",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_get_by_id",
            ),
        ))
        .await
        .unwrap();
    assert_eq!(apply_resp.status(), StatusCode::OK);
    let apply_json = read_json(apply_resp).await;
    let data_link_id = apply_json["data"]["data_link"]["data_link_id"]
        .as_str()
        .unwrap();

    let get_resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/datalink/v1/datalinks/{data_link_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_json = read_json(get_resp).await;
    assert!(get_json["data"]["data_link"].is_object());
    assert!(get_json["data"]["datasource"].is_object());
    assert!(get_json["data"]["etl_pipeline"].is_object());
    assert!(get_json["data"]["result_table"].is_object());
}

#[tokio::test]
async fn get_by_result_table_name_works() {
    let app = build_app();

    let apply_resp = app
        .clone()
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "get-by-table",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_get_by_table",
            ),
        ))
        .await
        .unwrap();
    assert_eq!(apply_resp.status(), StatusCode::OK);

    let get_resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks/by-result-table/rt_get_by_table")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);

    let body = read_json(get_resp).await;
    assert_eq!(
        body["data"]["result_table"]["result_table_name"],
        Value::String("rt_get_by_table".to_string())
    );
}

#[tokio::test]
async fn get_list_supports_all_filters_and_pagination_params() {
    let app = build_app();

    for payload in [
        apply_payload(
            "list-target",
            "security",
            "audit-gateway",
            "event",
            "active",
            "rt_list_target",
        ),
        apply_payload(
            "list-other",
            "message",
            "audit-gateway",
            "event",
            "active",
            "rt_list_other",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(make_json_request(
                Method::PUT,
                "/api/datalink/v1/datalinks:apply",
                payload,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks?domain=security&owner_service=audit-gateway&data_type=event&status=active&storage_type=mysql&page=1&page_size=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = read_json(response).await;
    assert_eq!(body["data"]["page"], Value::Number(1u64.into()));
    assert_eq!(body["data"]["page_size"], Value::Number(10u64.into()));
    assert_eq!(body["data"]["total"], Value::Number(1u64.into()));
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["data"]["items"][0]["result_table"]["result_table_name"],
        Value::String("rt_list_target".to_string())
    );
}

#[tokio::test]
async fn patch_status_enforces_disabled_reason_rules() {
    let app = build_app();

    let apply_resp = app
        .clone()
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            apply_payload(
                "patch-status",
                "message",
                "svc-a",
                "log",
                "draft",
                "rt_patch_status",
            ),
        ))
        .await
        .unwrap();
    assert_eq!(apply_resp.status(), StatusCode::OK);
    let apply_json = read_json(apply_resp).await;
    let data_link_id = apply_json["data"]["data_link"]["data_link_id"]
        .as_str()
        .unwrap();

    let bad_patch = app
        .clone()
        .oneshot(make_json_request(
            Method::PATCH,
            &format!("/api/datalink/v1/datalinks/{data_link_id}/status"),
            json!({"status":"disabled"}),
        ))
        .await
        .unwrap();
    assert_eq!(bad_patch.status(), StatusCode::BAD_REQUEST);
    let bad_json = read_json(bad_patch).await;
    assert_eq!(
        bad_json["error"]["code"],
        Value::String("DL_STATUS_MESSAGE_REQUIRED".to_string())
    );

    let ok_patch = app
        .oneshot(make_json_request(
            Method::PATCH,
            &format!("/api/datalink/v1/datalinks/{data_link_id}/status"),
            json!({"status":"disabled", "reason":"maintenance"}),
        ))
        .await
        .unwrap();
    assert_eq!(ok_patch.status(), StatusCode::OK);
    let ok_json = read_json(ok_patch).await;
    assert_eq!(
        ok_json["data"]["data_link"]["status_message"],
        Value::String("maintenance".to_string())
    );
}

#[tokio::test]
async fn error_responses_use_dl_code_envelope_shape() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks/not-exists")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await;
    assert_eq!(body["success"], Value::Bool(false));
    assert_eq!(
        body["error"]["code"],
        Value::String("DL_NOT_FOUND".to_string())
    );
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn malformed_request_and_query_rejections_use_error_envelope() {
    let app = build_app();

    let bad_json_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/datalink/v1/datalinks:apply")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bad-json",
                        "domain": "message",
                        "owner_service": "svc-a",
                        "data_type": "not-a-valid-type"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(bad_json_response.status(), StatusCode::BAD_REQUEST);
    let bad_json_body = read_json(bad_json_response).await;
    assert_eq!(bad_json_body["success"], Value::Bool(false));
    assert_eq!(
        bad_json_body["error"]["code"],
        Value::String("DL_INVALID_ARGUMENT".to_string())
    );

    let bad_query_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/datalink/v1/datalinks?page=oops&page_size=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(bad_query_response.status(), StatusCode::BAD_REQUEST);
    let bad_query_body = read_json(bad_query_response).await;
    assert_eq!(bad_query_body["success"], Value::Bool(false));
    assert_eq!(
        bad_query_body["error"]["code"],
        Value::String("DL_INVALID_ARGUMENT".to_string())
    );
}

#[tokio::test]
async fn oversized_request_body_uses_error_envelope() {
    let app = build_app();
    let huge_mapping = "x".repeat(2 * 1024 * 1024);
    let mut payload = apply_payload(
        "oversized-body",
        "message",
        "svc-a",
        "log",
        "draft",
        "rt_oversized_body",
    );
    payload["etl_pipeline"]["config"]["mapping"] = Value::String(huge_mapping);

    let response = app
        .oneshot(make_json_request(
            Method::PUT,
            "/api/datalink/v1/datalinks:apply",
            payload,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = read_json(response).await;
    assert_eq!(body["success"], Value::Bool(false));
    assert_eq!(
        body["error"]["code"],
        Value::String("DL_INVALID_ARGUMENT".to_string())
    );
}

#[test]
fn datalink_engine_config_supports_mysql_settings() {
    let raw = r#"
[datalink_engine]
backend = "mysql"

[datalink_engine.mysql]
host = "127.0.0.1"
port = 3306
user = "root"
password = "secret"
database = "rsde"
max_connections = 16
min_connections = 2
connect_timeout_secs = 5
"#;

    let parsed: GlobalConfig = toml::from_str(raw).expect("mysql datalink config should parse");
    let datalink = parsed
        .datalink_engine
        .expect("datalink_engine config exists");

    assert!(matches!(datalink.backend, DataLinkEngineBackend::Mysql));
    let mysql = datalink.mysql.expect("mysql settings should be present");
    assert_eq!(mysql.database, "rsde");
    assert_eq!(mysql.max_connections, 16);
}

#[test]
fn mysql_backend_requires_mysql_config() {
    let err = apiserver::build_datalink_v1_router(DataLinkEngineConfig {
        backend: DataLinkEngineBackend::Mysql,
        mysql: None,
    })
    .expect_err("mysql backend should require mysql config");

    assert!(
        err.to_string().contains("mysql") && err.to_string().contains("config"),
        "unexpected mysql config error: {err}"
    );
}
