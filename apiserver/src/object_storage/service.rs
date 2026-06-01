use crate::object_storage::dto::{
    CreateDirectoryResponse, CreateUploadTokenResponse, DeleteObjectFailure, DeleteObjectResponse,
    DeleteObjectsResponse, DownloadUrlResponse, HealthResponse, ListObjectsResponse,
    MoveObjectResponse, ObjectDetailResponse, ObjectItem, ObjectPrefix,
};
use crate::object_storage::error::{ObjectStorageError, Result};
use crate::object_storage::qiniu::{
    apply_path_prefix_constraint, ensure_directory_marker, join_directory_marker, normalize_key,
    normalize_prefix, strip_path_prefix_for_response,
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StoredObjectItem {
    pub key: String,
    pub size: Option<u64>,
    pub mime_type: Option<String>,
    pub updated_at: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredObjectDetail {
    pub key: String,
    pub size: Option<u64>,
    pub hash: Option<String>,
    pub mime_type: Option<String>,
    pub updated_at: Option<String>,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredListObjectsOutput {
    pub marker: Option<String>,
    pub has_more: bool,
    pub prefixes: Vec<String>,
    pub items: Vec<StoredObjectItem>,
}

#[async_trait]
pub trait ObjectStorageBackend: Send + Sync {
    async fn list_objects(
        &self,
        prefix: &str,
        marker: Option<&str>,
        limit: u16,
    ) -> Result<StoredListObjectsOutput>;
    async fn get_object_detail(&self, key: &str) -> Result<StoredObjectDetail>;
    async fn create_directory(&self, key: &str) -> Result<()>;
    async fn move_object(&self, from_key: &str, to_key: &str) -> Result<()>;
    async fn delete_object(&self, key: &str) -> Result<()>;
    fn create_upload_token(&self, key: &str, ttl_secs: u64) -> Result<String>;
    fn create_private_download_url(&self, url: &str, ttl_secs: u64) -> Result<String>;
    fn upload_url(&self) -> String;
}

#[derive(Clone)]
pub struct ObjectStorageService {
    config: Arc<config::object_storage::ObjectStorageConfig>,
    backend: Arc<dyn ObjectStorageBackend>,
}

impl ObjectStorageService {
    pub fn new(
        config: config::object_storage::ObjectStorageConfig,
        backend: Arc<dyn ObjectStorageBackend>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            backend,
        }
    }

    pub async fn health_check(&self) -> Result<HealthResponse> {
        Ok(HealthResponse {
            status: "ok".to_string(),
        })
    }

    pub async fn list_objects(
        &self,
        prefix: Option<String>,
        marker: Option<String>,
        limit: Option<u16>,
    ) -> Result<ListObjectsResponse> {
        let current_prefix = self.resolve_prefix(prefix.as_deref())?;
        let storage_prefix = self.resolve_storage_prefix(&current_prefix)?;
        let limit = limit.unwrap_or(100).clamp(1, 1000);
        let output = self
            .backend
            .list_objects(&storage_prefix, marker.as_deref(), limit)
            .await?;

        Ok(ListObjectsResponse {
            current_prefix: current_prefix.clone(),
            marker: output.marker,
            has_more: output.has_more,
            prefixes: self.to_visible_prefixes(&current_prefix, output.prefixes, &output.items),
            items: output
                .items
                .into_iter()
                .filter_map(|item| self.to_visible_item(&current_prefix, item))
                .collect(),
        })
    }

    pub async fn get_object_detail(&self, key: &str) -> Result<ObjectDetailResponse> {
        let stored_key = self.resolve_object_key(key)?;
        let detail = self.backend.get_object_detail(&stored_key).await?;
        let visible_key = self.visible_key(&detail.key);
        let download_url = self.try_create_download_url_for_stored_key(&detail.key)?;

        Ok(ObjectDetailResponse {
            name: object_name(&visible_key),
            is_directory: visible_key.ends_with('/'),
            key: visible_key,
            size: detail.size,
            hash: detail.hash,
            mime_type: detail.mime_type,
            updated_at: detail.updated_at,
            download_url,
            storage_class: detail.storage_class,
        })
    }

    pub async fn create_directory(
        &self,
        prefix: Option<&str>,
        name: &str,
    ) -> Result<CreateDirectoryResponse> {
        let current_prefix = self.resolve_prefix(prefix)?;
        let directory_name = normalize_prefix(name)?;
        let visible_key = join_directory_marker(&current_prefix, &directory_name);
        let stored_key = self.resolve_directory_key(&visible_key)?;
        self.backend.create_directory(&stored_key).await?;
        let visible_key = self.visible_key(&stored_key);

        Ok(CreateDirectoryResponse {
            name: object_name(&visible_key),
            key: visible_key,
            is_directory: true,
        })
    }

    pub async fn move_object(&self, from_key: &str, to_key: &str) -> Result<MoveObjectResponse> {
        ensure_non_empty_key(from_key, "from_key")?;
        ensure_non_empty_key(to_key, "to_key")?;

        let stored_from_key = self.resolve_object_key(from_key)?;
        let stored_to_key = self.resolve_object_key(to_key)?;
        self.backend
            .move_object(&stored_from_key, &stored_to_key)
            .await?;

        Ok(MoveObjectResponse {
            from_key: self.visible_key(&stored_from_key),
            to_key: self.visible_key(&stored_to_key),
        })
    }

    pub async fn delete_object(&self, key: &str) -> Result<DeleteObjectResponse> {
        ensure_non_empty_key(key, "key")?;

        let stored_key = self.resolve_object_key(key)?;
        self.backend.delete_object(&stored_key).await?;

        Ok(DeleteObjectResponse {
            deleted_key: self.visible_key(&stored_key),
        })
    }

    pub async fn delete_objects(&self, keys: Vec<String>) -> Result<DeleteObjectsResponse> {
        if keys.is_empty() {
            return Err(ObjectStorageError::InvalidInput(
                "keys cannot be empty".to_string(),
            ));
        }

        let mut deleted_keys = Vec::new();
        let mut failed = Vec::new();

        for key in keys {
            match self.delete_object(&key).await {
                Ok(response) => deleted_keys.push(response.deleted_key),
                Err(err) => failed.push(DeleteObjectFailure {
                    key,
                    error: err.to_string(),
                }),
            }
        }

        Ok(DeleteObjectsResponse {
            deleted_keys,
            failed,
        })
    }

    pub fn create_upload_token(
        &self,
        prefix: Option<&str>,
        filename: &str,
    ) -> Result<CreateUploadTokenResponse> {
        ensure_non_empty_key(filename, "filename")?;

        let current_prefix = self.resolve_prefix(prefix)?;
        let normalized_filename = normalize_key(filename)?;
        ensure_valid_object_key(&normalized_filename, "filename")?;
        let visible_key = join_directory_marker(&current_prefix, &normalized_filename);
        let stored_key = self.resolve_object_key(&visible_key)?;
        let upload_token = self
            .backend
            .create_upload_token(&stored_key, self.config.upload_token_ttl_secs)?;

        Ok(CreateUploadTokenResponse {
            upload_token,
            object_key: self.visible_key(&stored_key),
            upload_key: stored_key,
            upload_url: self.backend.upload_url(),
            expires_at: expires_at(self.config.upload_token_ttl_secs),
            bucket: self.config.bucket.clone(),
        })
    }

    pub fn create_download_url(&self, key: &str) -> Result<DownloadUrlResponse> {
        ensure_non_empty_key(key, "key")?;
        let normalized_key = normalize_key(key)?;
        ensure_valid_object_key(&normalized_key, "key")?;

        let stored_key = self.resolve_object_key(&normalized_key)?;
        let (download_url, expires_at) = self.create_download_url_parts(&stored_key)?;

        Ok(DownloadUrlResponse {
            key: self.visible_key(&stored_key),
            download_url,
            expires_at,
        })
    }

    pub fn resolve_object_key(&self, key: &str) -> Result<String> {
        let normalized_key = normalize_key(key)?;
        let resolved = if let Some(prefix) = self.config.normalized_path_prefix() {
            let joined = join_directory_marker(prefix.trim_end_matches('/'), &normalized_key);
            apply_path_prefix_constraint(&joined, &prefix)?;
            joined
        } else {
            normalized_key
        };

        Ok(resolved)
    }

    pub fn resolve_directory_key(&self, key: &str) -> Result<String> {
        let resolved = self.resolve_object_key(key)?;
        ensure_directory_marker(&resolved)
    }

    pub fn resolve_prefix(&self, prefix: Option<&str>) -> Result<String> {
        match prefix {
            Some(prefix) => {
                let normalized = normalize_prefix(prefix)?;
                if normalized.is_empty() {
                    Ok(String::new())
                } else {
                    Ok(format!("{normalized}/"))
                }
            }
            None => Ok(String::new()),
        }
    }

    pub fn resolve_storage_prefix(&self, prefix: &str) -> Result<String> {
        if prefix.is_empty() {
            if let Some(config_prefix) = self.config.normalized_path_prefix() {
                Ok(config_prefix)
            } else {
                Ok(String::new())
            }
        } else {
            self.resolve_directory_key(prefix)
        }
    }

    pub fn visible_key(&self, stored_key: &str) -> String {
        if let Some(prefix) = self.config.normalized_path_prefix() {
            strip_path_prefix_for_response(stored_key, &prefix)
        } else {
            stored_key.to_string()
        }
    }

    fn public_download_url(&self, stored_key: &str) -> Result<String> {
        let base_url = self
            .config
            .public_base_url
            .as_deref()
            .or(self.config.domain.as_deref())
            .ok_or_else(|| {
                ObjectStorageError::ConfigError(
                    "object_storage.public_base_url or object_storage.domain is required for download URLs"
                        .to_string(),
                )
            })?;
        let base_url = normalize_download_base_url(base_url, self.config.use_https);
        Ok(format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            percent_encode_path(stored_key)
        ))
    }

    fn create_download_url_parts(&self, stored_key: &str) -> Result<(String, Option<String>)> {
        let public_url = self.public_download_url(stored_key)?;

        if self.config.bucket_is_private {
            Ok((
                self.backend
                    .create_private_download_url(&public_url, self.config.private_url_ttl_secs)?,
                Some(expires_at(self.config.private_url_ttl_secs)),
            ))
        } else {
            Ok((public_url, None))
        }
    }

    fn try_create_download_url_for_stored_key(&self, stored_key: &str) -> Result<Option<String>> {
        match self.create_download_url_parts(stored_key) {
            Ok((download_url, _expires_at)) => Ok(Some(download_url)),
            Err(ObjectStorageError::ConfigError(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn to_visible_prefixes(
        &self,
        current_prefix: &str,
        stored_prefixes: Vec<String>,
        stored_items: &[StoredObjectItem],
    ) -> Vec<ObjectPrefix> {
        let mut prefixes = BTreeSet::new();

        for stored_prefix in stored_prefixes {
            let visible_key = self.visible_key(&stored_prefix);
            if visible_key.starts_with(current_prefix) && visible_key != current_prefix {
                prefixes.insert(first_child_prefix(current_prefix, &visible_key));
            }
        }

        for item in stored_items {
            let visible_key = self.visible_key(&item.key);
            if visible_key.starts_with(current_prefix)
                && visible_key != current_prefix
                && let Some(prefix) = child_prefix_from_key(current_prefix, &visible_key)
            {
                prefixes.insert(prefix);
            }
        }

        prefixes
            .into_iter()
            .map(|key| ObjectPrefix {
                name: object_name(&key),
                key,
                is_directory: true,
            })
            .collect()
    }

    fn to_visible_item(&self, current_prefix: &str, item: StoredObjectItem) -> Option<ObjectItem> {
        let visible_key = self.visible_key(&item.key);
        if visible_key.is_empty()
            || visible_key.ends_with('/')
            || !visible_key.starts_with(current_prefix)
        {
            return None;
        }

        let remainder = visible_key
            .strip_prefix(current_prefix)
            .unwrap_or(&visible_key);
        if remainder.is_empty() || remainder.contains('/') {
            return None;
        }

        Some(ObjectItem {
            name: object_name(&visible_key),
            key: visible_key,
            is_directory: false,
            size: item.size,
            mime_type: item.mime_type,
            updated_at: item.updated_at,
            hash: item.hash,
        })
    }
}

fn object_name(key: &str) -> String {
    key.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(key)
        .to_string()
}

fn ensure_non_empty_key(key: &str, field_name: &str) -> Result<()> {
    if key.trim().is_empty() {
        Err(ObjectStorageError::InvalidInput(format!(
            "{field_name} cannot be empty"
        )))
    } else {
        Ok(())
    }
}

fn ensure_valid_object_key(key: &str, field_name: &str) -> Result<()> {
    if key.is_empty() {
        Err(ObjectStorageError::InvalidInput(format!(
            "{field_name} cannot normalize to an empty key"
        )))
    } else if key.ends_with('/') {
        Err(ObjectStorageError::InvalidInput(format!(
            "{field_name} cannot be a directory marker"
        )))
    } else {
        Ok(())
    }
}

fn expires_at(ttl_secs: u64) -> String {
    (Utc::now() + ChronoDuration::seconds(ttl_secs as i64)).to_rfc3339()
}

fn normalize_download_base_url(base_url: &str, use_https: bool) -> String {
    if base_url.starts_with("http://") || base_url.starts_with("https://") {
        base_url.to_string()
    } else if use_https {
        format!("https://{base_url}")
    } else {
        format!("http://{base_url}")
    }
}

fn percent_encode_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode_segment(segment: &str) -> String {
    let mut encoded = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn child_prefix_from_key(current_prefix: &str, visible_key: &str) -> Option<String> {
    let remainder = visible_key.strip_prefix(current_prefix)?;
    let (first_segment, _) = remainder.split_once('/')?;
    if first_segment.is_empty() {
        None
    } else {
        Some(format!("{current_prefix}{first_segment}/"))
    }
}

fn first_child_prefix(current_prefix: &str, visible_key: &str) -> String {
    child_prefix_from_key(current_prefix, visible_key).unwrap_or_else(|| {
        if visible_key.ends_with('/') {
            visible_key.to_string()
        } else {
            format!("{visible_key}/")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ObjectStorageBackend, ObjectStorageService, StoredListObjectsOutput, StoredObjectDetail,
        StoredObjectItem,
    };
    use crate::object_storage::error::Result;
    use async_trait::async_trait;
    use std::sync::Arc;

    struct FakeBackend;

    #[async_trait]
    impl ObjectStorageBackend for FakeBackend {
        async fn list_objects(
            &self,
            _prefix: &str,
            marker: Option<&str>,
            _limit: u16,
        ) -> Result<StoredListObjectsOutput> {
            Ok(StoredListObjectsOutput {
                marker: marker.map(str::to_string),
                has_more: false,
                prefixes: Vec::new(),
                items: vec![
                    StoredObjectItem {
                        key: "team-a/images/demo.png".to_string(),
                        size: Some(42),
                        mime_type: Some("image/png".to_string()),
                        updated_at: None,
                        hash: Some("hash".to_string()),
                    },
                    StoredObjectItem {
                        key: "team-a/images/reports/".to_string(),
                        size: Some(0),
                        mime_type: None,
                        updated_at: None,
                        hash: None,
                    },
                    StoredObjectItem {
                        key: "team-a/images/reports/summary.pdf".to_string(),
                        size: Some(100),
                        mime_type: Some("application/pdf".to_string()),
                        updated_at: None,
                        hash: Some("hash2".to_string()),
                    },
                ],
            })
        }

        async fn get_object_detail(&self, key: &str) -> Result<StoredObjectDetail> {
            Ok(StoredObjectDetail {
                key: key.to_string(),
                size: Some(42),
                hash: Some("hash".to_string()),
                mime_type: Some("image/png".to_string()),
                updated_at: None,
                storage_class: None,
            })
        }

        async fn create_directory(&self, _key: &str) -> Result<()> {
            Ok(())
        }

        async fn move_object(&self, _from_key: &str, _to_key: &str) -> Result<()> {
            Ok(())
        }

        async fn delete_object(&self, _key: &str) -> Result<()> {
            Ok(())
        }

        fn create_upload_token(&self, key: &str, _ttl_secs: u64) -> Result<String> {
            Ok(format!("token:{key}"))
        }

        fn create_private_download_url(&self, url: &str, _ttl_secs: u64) -> Result<String> {
            Ok(format!("{url}?e=1770000000&token=ak:signature"))
        }

        fn upload_url(&self) -> String {
            "https://upload.example.com".to_string()
        }
    }

    fn test_service(path_prefix: Option<&str>) -> ObjectStorageService {
        ObjectStorageService::new(
            config::object_storage::ObjectStorageConfig {
                access_key: "ak".to_string(),
                secret_key: "sk".to_string(),
                bucket: "bucket".to_string(),
                region: "z0".to_string(),
                domain: Some("example.com".to_string()),
                public_base_url: None,
                upload_token_ttl_secs: 3600,
                private_url_ttl_secs: 3600,
                use_https: true,
                path_prefix: path_prefix.map(str::to_string),
                bucket_is_private: false,
            },
            Arc::new(FakeBackend),
        )
    }

    #[test]
    fn scoped_key_applies_normalized_config_prefix() {
        let service = test_service(Some("/team-a"));
        let resolved = service.resolve_object_key("images/demo.png").unwrap();
        assert_eq!(resolved, "team-a/images/demo.png");
    }

    #[test]
    fn scoped_key_drops_dot_segments_before_joining_prefix() {
        let service = test_service(Some("/team-a"));
        let resolved = service.resolve_object_key("./images/./demo.png").unwrap();
        assert_eq!(resolved, "team-a/images/demo.png");
    }

    #[test]
    fn scoped_directory_key_enforces_directory_marker_suffix() {
        let service = test_service(Some("/team-a"));
        let resolved = service.resolve_directory_key("images/reports").unwrap();
        assert_eq!(resolved, "team-a/images/reports/");
    }

    #[test]
    fn visible_key_strips_normalized_config_prefix() {
        let service = test_service(Some("/team-a"));
        assert_eq!(
            service.visible_key("team-a/images/demo.png"),
            "images/demo.png"
        );
    }

    #[test]
    fn storage_prefix_uses_config_prefix_at_root() {
        let service = test_service(Some("//team-a//./images///"));
        let resolved = service.resolve_storage_prefix("").unwrap();
        assert_eq!(resolved, "team-a/images/");
    }

    #[test]
    fn storage_prefix_combines_config_prefix_and_request_prefix() {
        let service = test_service(Some("/team-a"));
        let visible_prefix = service.resolve_prefix(Some("images/2026/")).unwrap();
        let resolved = service.resolve_storage_prefix(&visible_prefix).unwrap();
        assert_eq!(resolved, "team-a/images/2026/");
    }

    #[tokio::test]
    async fn directory_creation_returns_visible_directory_key() {
        let service = test_service(Some("/team-a"));
        let created = service
            .create_directory(Some("images/2026/"), "reports")
            .await
            .unwrap();
        assert_eq!(created.key, "images/2026/reports/");
        assert_eq!(created.name, "reports");
        assert!(created.is_directory);
    }

    #[tokio::test]
    async fn object_detail_uses_backend_metadata() {
        let service = test_service(Some("/team-a"));
        let detail = service.get_object_detail("images/demo.png").await.unwrap();
        assert_eq!(detail.key, "images/demo.png");
        assert_eq!(detail.size, Some(42));
        assert_eq!(detail.hash, Some("hash".to_string()));
        assert_eq!(detail.mime_type, Some("image/png".to_string()));
        assert_eq!(
            detail.download_url,
            Some("https://example.com/team-a/images/demo.png".to_string())
        );
    }

    #[tokio::test]
    async fn object_detail_signs_download_url_for_private_bucket() {
        let service = ObjectStorageService::new(
            config::object_storage::ObjectStorageConfig {
                access_key: "ak".to_string(),
                secret_key: "sk".to_string(),
                bucket: "bucket".to_string(),
                region: "z0".to_string(),
                domain: Some("example.com".to_string()),
                public_base_url: None,
                upload_token_ttl_secs: 3600,
                private_url_ttl_secs: 3600,
                use_https: true,
                path_prefix: Some("/team-a".to_string()),
                bucket_is_private: true,
            },
            Arc::new(FakeBackend),
        );

        let detail = service.get_object_detail("images/demo.png").await.unwrap();
        let download_url = detail.download_url.expect("download url");

        assert!(download_url.contains("https://example.com/team-a/images/demo.png"));
        assert!(download_url.contains("?e=1770000000&token=ak:signature"));
    }

    #[tokio::test]
    async fn move_object_returns_visible_keys() {
        let service = test_service(Some("/team-a"));
        let moved = service
            .move_object("images/old.png", "archive/old.png")
            .await
            .unwrap();
        assert_eq!(moved.from_key, "images/old.png");
        assert_eq!(moved.to_key, "archive/old.png");
    }

    #[tokio::test]
    async fn delete_object_returns_visible_deleted_key() {
        let service = test_service(Some("/team-a"));
        let deleted = service.delete_object("images/old.png").await.unwrap();
        assert_eq!(deleted.deleted_key, "images/old.png");
    }

    #[test]
    fn upload_token_returns_exact_storage_key_for_form_upload() {
        let service = test_service(Some("/team-a"));
        let token = service
            .create_upload_token(Some("images/2026/"), "demo.png")
            .unwrap();

        assert_eq!(token.object_key, "images/2026/demo.png");
        assert_eq!(token.upload_key, "team-a/images/2026/demo.png");
        assert_eq!(token.upload_token, "token:team-a/images/2026/demo.png");
    }

    #[tokio::test]
    async fn list_objects_splits_files_and_child_prefixes() {
        let service = test_service(Some("/team-a"));
        let output = service
            .list_objects(
                Some("images/".to_string()),
                Some("m1".to_string()),
                Some(20),
            )
            .await
            .unwrap();

        assert_eq!(output.current_prefix, "images/");
        assert_eq!(output.marker, Some("m1".to_string()));
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].key, "images/demo.png");
        assert_eq!(output.items[0].size, Some(42));
        assert_eq!(output.prefixes.len(), 1);
        assert_eq!(output.prefixes[0].key, "images/reports/");
        assert!(output.prefixes[0].is_directory);
    }
}
