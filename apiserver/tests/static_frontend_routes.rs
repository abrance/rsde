use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::util::ServiceExt;

struct TestFrontendDir {
    path: PathBuf,
}

impl TestFrontendDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rsde-frontend-test-{unique}"));

        fs::create_dir_all(path.join("assets")).expect("create frontend test dir");
        fs::write(
            path.join("index.html"),
            r#"<!doctype html><html><body><div id="root">rsde spa shell</div></body></html>"#,
        )
        .expect("write index.html");
        fs::write(path.join("assets/app.js"), "console.log('asset loaded');").expect("write asset");

        Self { path }
    }

    fn as_path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestFrontendDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn object_storage_config() -> config::object_storage::ObjectStorageConfig {
    config::object_storage::ObjectStorageConfig {
        access_key: "test_key".to_string(),
        secret_key: "test_secret".to_string(),
        bucket: "test_bucket".to_string(),
        region: "z0".to_string(),
        domain: Some("test.example.com".to_string()),
        public_base_url: None,
        upload_token_ttl_secs: 3600,
        private_url_ttl_secs: 3600,
        use_https: true,
        path_prefix: None,
        bucket_is_private: false,
    }
}

fn request(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request")
}

async fn response_body(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");

    String::from_utf8(body.to_vec()).expect("utf-8 body")
}

#[tokio::test]
async fn spa_page_routes_fall_back_to_index_html() {
    let frontend_dir = TestFrontendDir::new();
    let app = apiserver::build_frontend_router(
        frontend_dir
            .as_path()
            .to_str()
            .expect("frontend path must be utf-8"),
    );

    let response = app
        .oneshot(request("/object-storage"))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html"
    );

    let body = response_body(response).await;
    assert!(body.contains("rsde spa shell"));
}

#[tokio::test]
async fn frontend_assets_are_served_from_assets_directory() {
    let frontend_dir = TestFrontendDir::new();
    let app = apiserver::build_frontend_router(
        frontend_dir
            .as_path()
            .to_str()
            .expect("frontend path must be utf-8"),
    );

    let response = app
        .oneshot(request("/assets/app.js"))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body(response).await;
    assert!(body.contains("asset loaded"));
}

#[tokio::test]
async fn missing_frontend_assets_return_not_found() {
    let frontend_dir = TestFrontendDir::new();
    let app = apiserver::build_frontend_router(
        frontend_dir
            .as_path()
            .to_str()
            .expect("frontend path must be utf-8"),
    );

    let response = app
        .oneshot(request("/assets/missing.js"))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn composed_app_keeps_api_routes_while_spa_routes_fall_back() {
    let frontend_dir = TestFrontendDir::new();
    let app = Router::new()
        .nest(
            "/api/object-storage",
            apiserver::object_storage::create_routes(object_storage_config()),
        )
        .merge(apiserver::build_frontend_router(
            frontend_dir
                .as_path()
                .to_str()
                .expect("frontend path must be utf-8"),
        ));

    let api_response = app
        .clone()
        .oneshot(request("/api/object-storage/health"))
        .await
        .expect("api response");
    assert_eq!(api_response.status(), StatusCode::OK);
    assert_eq!(
        api_response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let api_body = response_body(api_response).await;
    assert!(api_body.contains(r#""status":"ok""#));

    let page_response = app
        .clone()
        .oneshot(request("/object-storage"))
        .await
        .expect("page response");
    assert_eq!(page_response.status(), StatusCode::OK);
    assert_eq!(
        page_response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html"
    );
    let page_body = response_body(page_response).await;
    assert!(page_body.contains("rsde spa shell"));

    let asset_response = app
        .oneshot(request("/assets/app.js"))
        .await
        .expect("asset response");
    assert_eq!(asset_response.status(), StatusCode::OK);
    let asset_body = response_body(asset_response).await;
    assert!(asset_body.contains("asset loaded"));
}
