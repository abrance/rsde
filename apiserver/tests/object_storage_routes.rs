fn test_config() -> config::object_storage::ObjectStorageConfig {
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

fn private_bucket_config() -> config::object_storage::ObjectStorageConfig {
    config::object_storage::ObjectStorageConfig {
        bucket_is_private: true,
        ..test_config()
    }
}

fn public_base_url_config() -> config::object_storage::ObjectStorageConfig {
    config::object_storage::ObjectStorageConfig {
        domain: None,
        public_base_url: Some("https://cdn.example.com/base".to_string()),
        bucket_is_private: false,
        ..test_config()
    }
}

async fn spawn_object_storage_app() -> String {
    spawn_object_storage_app_with_config(test_config()).await
}

async fn spawn_object_storage_app_with_config(
    config: config::object_storage::ObjectStorageConfig,
) -> String {
    let app = apiserver::object_storage::create_routes_with_backend(config, Arc::new(FakeBackend));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{addr}")
}

#[tokio::test]
async fn object_storage_health_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/health"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["status"], "ok");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_objects_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/objects?prefix=images/&limit=20"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["current_prefix"], "images/");
    assert_eq!(body["data"]["marker"], "next-marker");
    assert_eq!(body["data"]["has_more"], true);
    assert!(body["data"]["items"].is_array());
    assert!(body["data"]["prefixes"].is_array());
    assert_eq!(body["data"]["items"][0]["key"], "images/demo.png");
    assert_eq!(body["data"]["prefixes"][0]["key"], "images/reports/");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_objects_rejects_invalid_prefix_as_bad_request() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/objects?prefix=../secret"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains(".."));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_detail_rejects_invalid_key_as_bad_request() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/objects/detail?key=../secret.txt"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains(".."));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_detail_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/objects/detail?key=images/demo.png"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["key"], "images/demo.png");
    assert_eq!(body["data"]["name"], "demo.png");
    assert_eq!(body["data"]["is_directory"], false);
    assert_eq!(
        body["data"]["download_url"],
        "https://test.example.com/images/demo.png"
    );
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_detail_private_download_url_is_signed() {
    let base_url = spawn_object_storage_app_with_config(private_bucket_config()).await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/objects/detail?key=images/demo.png"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["key"], "images/demo.png");
    assert!(
        body["data"]["download_url"]
            .as_str()
            .unwrap()
            .contains("?e=")
    );
    assert!(
        body["data"]["download_url"]
            .as_str()
            .unwrap()
            .contains("token=")
    );
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_directories_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/directories"))
        .json(&serde_json::json!({
            "prefix": "images/2026/",
            "name": "reports"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["key"], "images/2026/reports/");
    assert_eq!(body["data"]["is_directory"], true);
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_move_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/move"))
        .json(&serde_json::json!({
            "from_key": "images/old.png",
            "to_key": "archive/old.png"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["from_key"], "images/old.png");
    assert_eq!(body["data"]["to_key"], "archive/old.png");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_move_conflict_returns_409() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/move"))
        .json(&serde_json::json!({
            "from_key": "images/old.png",
            "to_key": "archive/conflict.png"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 409);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "object_conflict");
}

#[tokio::test]
async fn object_storage_move_rejects_empty_keys() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/move"))
        .json(&serde_json::json!({
            "from_key": "",
            "to_key": "archive/old.png"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("empty"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_delete_returns_success_envelope() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/delete"))
        .json(&serde_json::json!({
            "key": "images/old.png"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["deleted_key"], "images/old.png");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_delete_rejects_empty_key() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/delete"))
        .json(&serde_json::json!({
            "key": ""
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("empty"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_delete_batch_returns_deleted_and_failed_keys() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/delete-batch"))
        .json(&serde_json::json!({
            "keys": ["images/old.png", "images/fail.png"]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["deleted_keys"][0], "images/old.png");
    assert_eq!(body["data"]["failed"][0]["key"], "images/fail.png");
    assert!(
        body["data"]["failed"][0]["error"]
            .as_str()
            .unwrap()
            .contains("failed")
    );
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_delete_batch_reports_empty_key_failure() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/objects/delete-batch"))
        .json(&serde_json::json!({
            "keys": ["images/old.png", ""]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(
        body["data"]["deleted_keys"],
        serde_json::json!(["images/old.png"])
    );
    assert_eq!(body["data"]["failed"][0]["key"], "");
    assert!(
        body["data"]["failed"][0]["error"]
            .as_str()
            .unwrap()
            .contains("empty")
    );
}

#[tokio::test]
async fn object_storage_upload_token_returns_expected_fields() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/upload-token"))
        .json(&serde_json::json!({
            "prefix": "images/2026/",
            "filename": "demo.png"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert!(
        body["data"]["upload_token"]
            .as_str()
            .unwrap()
            .contains("token:")
    );
    assert_eq!(body["data"]["object_key"], "images/2026/demo.png");
    assert_eq!(body["data"]["upload_url"], "https://upload.example.com");
    assert!(body["data"]["expires_at"].as_str().is_some());
    assert_eq!(body["data"]["bucket"], "test_bucket");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}

#[tokio::test]
async fn object_storage_upload_token_rejects_dot_only_filename() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/upload-token"))
        .json(&serde_json::json!({
            "prefix": "images/2026/",
            "filename": "./"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("empty"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_upload_token_rejects_directory_marker_filename() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/upload-token"))
        .json(&serde_json::json!({
            "prefix": "images/2026/",
            "filename": "reports/"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("directory"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_public_download_url_uses_public_base_url() {
    let base_url = spawn_object_storage_app_with_config(public_base_url_config()).await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/download-url?key=images/demo.png"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["key"], "images/demo.png");
    assert_eq!(
        body["data"]["download_url"],
        "https://cdn.example.com/base/images/demo.png"
    );
    assert_eq!(body["data"]["expires_at"], serde_json::Value::Null);
}

#[tokio::test]
async fn object_storage_download_url_rejects_dot_only_key() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/download-url?key=./"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], false);
    assert!(body["error"].as_str().unwrap().contains("empty"));
    assert_eq!(body["code"], 400);
}

#[tokio::test]
async fn object_storage_private_download_url_is_signed() {
    let base_url = spawn_object_storage_app_with_config(private_bucket_config()).await;

    let response = reqwest::Client::new()
        .get(format!("{base_url}/download-url?key=images/demo.png"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["key"], "images/demo.png");
    assert!(
        body["data"]["download_url"]
            .as_str()
            .unwrap()
            .contains("?e=")
    );
    assert!(
        body["data"]["download_url"]
            .as_str()
            .unwrap()
            .contains("token=")
    );
    assert!(body["data"]["expires_at"].as_str().is_some());
}

#[tokio::test]
async fn object_storage_objects_clamps_limit_and_forwards_marker() {
    let base_url = spawn_object_storage_app().await;

    let response = reqwest::Client::new()
        .get(format!(
            "{base_url}/objects?prefix=images/&marker=custom-marker&limit=5000"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["marker"], "custom-marker:1000");
    assert_eq!(body.get("error"), None);
    assert_eq!(body.get("code"), None);
}
use apiserver::object_storage::error::ObjectStorageError;
use apiserver::object_storage::service::{
    ObjectStorageBackend, StoredListObjectsOutput, StoredObjectDetail, StoredObjectItem,
};
use async_trait::async_trait;
use std::sync::Arc;

struct FakeBackend;

#[async_trait]
impl ObjectStorageBackend for FakeBackend {
    async fn list_objects(
        &self,
        _prefix: &str,
        marker: Option<&str>,
        limit: u16,
    ) -> apiserver::object_storage::error::Result<StoredListObjectsOutput> {
        Ok(StoredListObjectsOutput {
            marker: Some(
                marker
                    .map(|marker| format!("{marker}:{limit}"))
                    .unwrap_or_else(|| "next-marker".to_string()),
            ),
            has_more: true,
            prefixes: vec!["images/reports/".to_string()],
            items: vec![StoredObjectItem {
                key: "images/demo.png".to_string(),
                size: Some(42),
                mime_type: Some("image/png".to_string()),
                updated_at: None,
                hash: Some("hash".to_string()),
            }],
        })
    }

    async fn get_object_detail(
        &self,
        key: &str,
    ) -> apiserver::object_storage::error::Result<StoredObjectDetail> {
        Ok(StoredObjectDetail {
            key: key.to_string(),
            size: None,
            hash: None,
            mime_type: None,
            updated_at: None,
            storage_class: None,
        })
    }

    async fn create_directory(&self, _key: &str) -> apiserver::object_storage::error::Result<()> {
        Ok(())
    }

    async fn move_object(
        &self,
        _from_key: &str,
        to_key: &str,
    ) -> apiserver::object_storage::error::Result<()> {
        if to_key == "archive/conflict.png" {
            Err(ObjectStorageError::ObjectConflict(
                "object already exists".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    async fn delete_object(&self, key: &str) -> apiserver::object_storage::error::Result<()> {
        if key == "images/fail.png" {
            Err(ObjectStorageError::DeleteError(
                "failed to delete object".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn create_upload_token(
        &self,
        key: &str,
        _ttl_secs: u64,
    ) -> apiserver::object_storage::error::Result<String> {
        Ok(format!("token:{key}"))
    }

    fn create_private_download_url(
        &self,
        url: &str,
        _ttl_secs: u64,
    ) -> apiserver::object_storage::error::Result<String> {
        Ok(format!("{url}?e=1770000000&token=ak:signature"))
    }

    fn upload_url(&self) -> String {
        "https://upload.example.com".to_string()
    }
}
