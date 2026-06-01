/// Qiniu object storage integration and path normalization helpers.
use crate::object_storage::error::{ObjectStorageError, Result};
use crate::object_storage::service::{
    ObjectStorageBackend, StoredListObjectsOutput, StoredObjectDetail, StoredObjectItem,
};
use async_trait::async_trait;
use futures::{io::Cursor, stream::TryStreamExt};
use qiniu_sdk::http::StatusCode;
use qiniu_sdk::http_client::{ResponseError, ResponseErrorKind};
use qiniu_sdk::objects::{ObjectsManager, apis::credential::Credential};
use qiniu_sdk::upload::{AutoUploader, AutoUploaderObjectParams, UploadManager, UploadTokenSigner};
use qiniu_sdk::upload_token::{ObjectUploadTokenProvider, ToStringOptions, UploadTokenProvider};
use std::time::Duration;

#[derive(Clone)]
pub struct QiniuObjectStorageBackend {
    bucket_name: String,
    region: String,
    use_https: bool,
    credential: Credential,
    objects_manager: ObjectsManager,
    upload_manager: UploadManager,
}

impl QiniuObjectStorageBackend {
    pub fn new(config: &config::object_storage::ObjectStorageConfig) -> Self {
        let credential = Credential::new(config.access_key.clone(), config.secret_key.clone());
        let objects_manager = ObjectsManager::builder(credential.clone())
            .use_https(config.use_https)
            .build();
        let upload_manager = UploadManager::builder(UploadTokenSigner::new_credential_provider(
            credential.clone(),
            config.bucket.clone(),
            Duration::from_secs(config.upload_token_ttl_secs),
        ))
        .use_https(config.use_https)
        .build();

        Self {
            bucket_name: config.bucket.clone(),
            region: config.region.clone(),
            use_https: config.use_https,
            credential,
            objects_manager,
            upload_manager,
        }
    }

    fn bucket(&self) -> qiniu_sdk::objects::Bucket {
        self.objects_manager.bucket(self.bucket_name.clone())
    }
}

#[async_trait]
impl ObjectStorageBackend for QiniuObjectStorageBackend {
    async fn list_objects(
        &self,
        prefix: &str,
        marker: Option<&str>,
        limit: u16,
    ) -> Result<StoredListObjectsOutput> {
        let bucket = self.bucket();
        let mut builder = bucket.list();
        builder.limit(usize::from(limit));
        if !prefix.is_empty() {
            builder.prefix(prefix.to_string());
        }
        if let Some(marker) = marker.filter(|marker| !marker.is_empty()) {
            builder.marker(marker.to_string());
        }

        let mut stream = builder.stream();
        let mut items = Vec::new();
        while let Some(entry) = stream.try_next().await.map_err(storage_error)? {
            items.push(StoredObjectItem {
                key: entry.get_key_as_str().to_string(),
                size: Some(entry.get_size_as_u64()),
                mime_type: Some(entry.get_mime_type_as_str().to_string()),
                updated_at: Some(entry.get_put_time_as_u64().to_string()),
                hash: Some(entry.get_hash_as_str().to_string()),
            });
        }

        Ok(StoredListObjectsOutput {
            marker: stream.marker().map(str::to_string),
            has_more: stream.marker().is_some_and(|marker| !marker.is_empty()),
            prefixes: Vec::new(),
            items,
        })
    }

    async fn get_object_detail(&self, key: &str) -> Result<StoredObjectDetail> {
        let response = self
            .bucket()
            .stat_object(key)
            .async_call()
            .await
            .map_err(storage_error)?;
        let body = response.into_body();

        Ok(StoredObjectDetail {
            key: key.to_string(),
            size: Some(body.get_size_as_u64()),
            hash: Some(body.get_hash_as_str().to_string()),
            mime_type: Some(body.get_mime_type_as_str().to_string()),
            updated_at: Some(body.get_put_time_as_u64().to_string()),
            storage_class: Some(body.get_type_as_u64().to_string()),
        })
    }

    async fn create_directory(&self, key: &str) -> Result<()> {
        let params = AutoUploaderObjectParams::builder()
            .object_name(key.to_string())
            .file_name(key.to_string())
            .build();
        let uploader: AutoUploader = self.upload_manager.auto_uploader();
        uploader
            .async_upload_reader(Cursor::new(Vec::<u8>::new()), params)
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn move_object(&self, from_key: &str, to_key: &str) -> Result<()> {
        self.bucket()
            .move_object_to(from_key, &self.bucket_name, to_key)
            .async_call()
            .await
            .map_err(move_error)?;
        Ok(())
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        self.bucket()
            .delete_object(key)
            .async_call()
            .await
            .map_err(delete_error)?;
        Ok(())
    }

    fn create_upload_token(&self, key: &str, ttl_secs: u64) -> Result<String> {
        ObjectUploadTokenProvider::new(
            self.bucket_name.clone(),
            key.to_string(),
            Duration::from_secs(ttl_secs),
            self.credential.clone(),
        )
        .to_token_string(ToStringOptions::default())
        .map(|token| token.into_owned())
        .map_err(|err| ObjectStorageError::UploadError(err.to_string()))
    }

    fn create_private_download_url(&self, url: &str, ttl_secs: u64) -> Result<String> {
        let uri = url.parse().map_err(|err| {
            ObjectStorageError::DownloadError(format!("invalid download url: {err}"))
        })?;
        Ok(self
            .credential
            .sign_download_url(uri, Duration::from_secs(ttl_secs))
            .to_string())
    }

    fn upload_url(&self) -> String {
        let scheme = if self.use_https { "https" } else { "http" };
        format!("{}://{}", scheme, qiniu_upload_host(&self.region))
    }
}

fn qiniu_upload_host(region: &str) -> &'static str {
    match region {
        "z0" => "up-z0.qiniup.com",
        "z1" => "up-z1.qiniup.com",
        "z2" => "up-z2.qiniup.com",
        "na0" => "up-na0.qiniup.com",
        "as0" => "up-as0.qiniup.com",
        _ => "up.qiniup.com",
    }
}

fn storage_error(err: impl std::fmt::Display) -> ObjectStorageError {
    ObjectStorageError::StorageError(err.to_string())
}

fn move_error(err: ResponseError) -> ObjectStorageError {
    if response_status_code(&err).is_some_and(|status_code| status_code == StatusCode::CONFLICT)
        || response_status_code(&err).is_some_and(|status_code| status_code.as_u16() == 614)
    {
        ObjectStorageError::ObjectConflict(err.to_string())
    } else {
        ObjectStorageError::StorageError(err.to_string())
    }
}

fn delete_error(err: ResponseError) -> ObjectStorageError {
    if response_status_code(&err).is_some_and(|status_code| status_code == StatusCode::NOT_FOUND) {
        ObjectStorageError::NotFound(err.to_string())
    } else {
        ObjectStorageError::DeleteError(err.to_string())
    }
}

fn response_status_code(err: &ResponseError) -> Option<StatusCode> {
    match err.kind() {
        ResponseErrorKind::StatusCodeError(status_code)
        | ResponseErrorKind::UnexpectedStatusCode(status_code) => Some(status_code),
        _ => None,
    }
}

fn normalize_constraint_prefix(prefix: &str) -> String {
    prefix.trim_end_matches('/').to_string()
}

fn normalize_segments(path: &str) -> Result<Vec<&str>> {
    let mut segments = Vec::new();

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if is_path_segment_double_dot(segment) {
            return Err(ObjectStorageError::ConfigError(
                "Path cannot contain .. path segments".to_string(),
            ));
        }

        segments.push(segment);
    }

    Ok(segments)
}

fn collapse_slashes(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_slash = false;

    for c in s.chars() {
        if c == '/' {
            if !prev_was_slash {
                result.push(c);
                prev_was_slash = true;
            }
        } else {
            result.push(c);
            prev_was_slash = false;
        }
    }

    result
}

fn is_path_segment_double_dot(segment: &str) -> bool {
    segment == ".."
}

fn contains_double_dot_segment(path: &str) -> bool {
    path.split('/').any(is_path_segment_double_dot)
}

/// Normalizes a prefix path for object storage.
/// - Rejects leading slashes
/// - Rejects .. as a path segment (but allows ".." within filenames like "file..txt")
/// - Collapses repeated slashes
/// - Trims trailing slashes
/// - Returns normalized prefix (may be empty string for root)
pub fn normalize_prefix(prefix: &str) -> Result<String> {
    if prefix.starts_with('/') {
        return Err(ObjectStorageError::InvalidInput(
            "Prefix cannot start with /".to_string(),
        ));
    }

    if contains_double_dot_segment(prefix) {
        return Err(ObjectStorageError::InvalidInput(
            "Prefix cannot contain .. path segments".to_string(),
        ));
    }

    let normalized = collapse_slashes(prefix);
    let normalized = normalize_segments(&normalized)?.join("/");

    Ok(normalized)
}

/// Normalizes a key (object name) for storage.
/// - Rejects leading slashes
/// - Rejects .. as a path segment (but allows ".." within filenames like "file..txt")
/// - Collapses repeated slashes
/// - Returns normalized key
pub fn normalize_key(key: &str) -> Result<String> {
    if key.starts_with('/') {
        return Err(ObjectStorageError::InvalidInput(
            "Key cannot start with /".to_string(),
        ));
    }

    if contains_double_dot_segment(key) {
        return Err(ObjectStorageError::InvalidInput(
            "Key cannot contain .. path segments".to_string(),
        ));
    }

    let normalized = collapse_slashes(key);
    let ends_with_slash = normalized.ends_with('/');
    let mut normalized = normalize_segments(&normalized)?.join("/");

    if ends_with_slash && !normalized.is_empty() {
        normalized.push('/');
    }

    Ok(normalized)
}

/// Ensures a logical directory key always uses a trailing slash marker.
pub fn ensure_directory_marker(key: &str) -> Result<String> {
    let normalized = normalize_key(key)?;

    if normalized.is_empty() {
        return Ok(normalized);
    }

    if normalized.ends_with('/') {
        Ok(normalized)
    } else {
        Ok(format!("{normalized}/"))
    }
}

/// Joins a prefix and key with proper directory marker semantics.
/// - If prefix is empty, returns key as-is
/// - If prefix is non-empty, joins with / separator
/// - Collapses any resulting double slashes
/// - If key ends with /, result preserves directory marker semantics
pub fn join_directory_marker(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        let joined = format!("{prefix}/{key}");
        collapse_slashes(&joined)
    }
}

/// Applies path_prefix boundary constraints to a key.
/// - Validates that key starts with prefix (if prefix is non-empty)
/// - Returns error if key does not respect prefix boundary
/// - Used to enforce that operations stay within configured prefix
pub fn apply_path_prefix_constraint(key: &str, prefix: &str) -> Result<()> {
    let prefix = normalize_constraint_prefix(prefix);

    if prefix.is_empty() {
        return Ok(());
    }

    if key == prefix || key.starts_with(&format!("{prefix}/")) {
        Ok(())
    } else {
        Err(ObjectStorageError::InvalidInput(format!(
            "Key '{key}' does not respect path_prefix '{prefix}'"
        )))
    }
}

/// Strips a path prefix from a key for response.
/// - If key starts with prefix/, removes the prefix and /
/// - If key equals prefix, returns empty string
/// - Otherwise returns key unchanged
pub fn strip_path_prefix_for_response(key: &str, prefix: &str) -> String {
    let prefix = normalize_constraint_prefix(prefix);

    if prefix.is_empty() {
        return key.to_string();
    }

    if key == prefix {
        String::new()
    } else if key.starts_with(&format!("{prefix}/")) {
        key[prefix.len() + 1..].to_string()
    } else {
        key.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============ normalize_prefix tests ============

    #[test]
    fn test_normalize_prefix_empty() {
        let result = normalize_prefix("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn qiniu_upload_url_uses_configured_region() {
        let config = config::object_storage::ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z2".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        let backend = QiniuObjectStorageBackend::new(&config);

        assert_eq!(backend.upload_url(), "https://up-z2.qiniup.com");
    }

    #[test]
    fn test_normalize_prefix_simple() {
        let result = normalize_prefix("my/prefix").unwrap();
        assert_eq!(result, "my/prefix");
    }

    #[test]
    fn test_normalize_prefix_rejects_leading_slash() {
        let result = normalize_prefix("/my/prefix");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot start with /")
        );
    }

    #[test]
    fn test_normalize_prefix_rejects_double_dot_segment() {
        let result = normalize_prefix("my/../prefix");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot contain .. path segments")
        );
    }

    #[test]
    fn test_normalize_prefix_rejects_double_dot_at_start() {
        let result = normalize_prefix("../prefix");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_prefix_allows_double_dot_in_filename() {
        let result = normalize_prefix("my/file..txt").unwrap();
        assert_eq!(result, "my/file..txt");
    }

    #[test]
    fn test_normalize_prefix_normalizes_double_slashes() {
        let result = normalize_prefix("my//prefix").unwrap();
        assert_eq!(result, "my/prefix");
    }

    #[test]
    fn test_normalize_prefix_collapses_multi_slash_runs() {
        let result = normalize_prefix("my////prefix").unwrap();
        assert_eq!(result, "my/prefix");
    }

    #[test]
    fn test_normalize_prefix_trims_trailing_slash() {
        let result = normalize_prefix("my/prefix/").unwrap();
        assert_eq!(result, "my/prefix");
    }

    #[test]
    fn test_normalize_prefix_multiple_trailing_slashes() {
        let result = normalize_prefix("my/prefix///").unwrap();
        assert_eq!(result, "my/prefix");
    }

    #[test]
    fn test_normalize_prefix_drops_dot_segments() {
        let result = normalize_prefix("team-a/./images/./2026/").unwrap();
        assert_eq!(result, "team-a/images/2026");
    }

    // ============ normalize_key tests ============

    #[test]
    fn test_normalize_key_simple() {
        let result = normalize_key("my/key").unwrap();
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_normalize_key_rejects_leading_slash() {
        let result = normalize_key("/my/key");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot start with /")
        );
    }

    #[test]
    fn test_normalize_key_rejects_double_dot_segment() {
        let result = normalize_key("my/../key");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot contain .. path segments")
        );
    }

    #[test]
    fn test_normalize_key_allows_double_dot_in_filename() {
        let result = normalize_key("my/file..txt").unwrap();
        assert_eq!(result, "my/file..txt");
    }

    #[test]
    fn test_normalize_key_normalizes_double_slashes() {
        let result = normalize_key("my//key").unwrap();
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_normalize_key_collapses_multi_slash_runs() {
        let result = normalize_key("my////key").unwrap();
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_normalize_key_with_trailing_slash() {
        let result = normalize_key("my/key/").unwrap();
        assert_eq!(result, "my/key/");
    }

    #[test]
    fn test_normalize_key_drops_dot_segments() {
        let result = normalize_key("team-a/./demo.png").unwrap();
        assert_eq!(result, "team-a/demo.png");
    }

    // ============ join_directory_marker tests ============

    #[test]
    fn test_join_directory_marker_empty_prefix() {
        let result = join_directory_marker("", "my/key");
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_join_directory_marker_with_prefix() {
        let result = join_directory_marker("prefix", "my/key");
        assert_eq!(result, "prefix/my/key");
    }

    #[test]
    fn test_join_directory_marker_no_double_slash_with_trailing_prefix() {
        let result = join_directory_marker("prefix/", "my/key");
        assert_eq!(result, "prefix/my/key");
    }

    #[test]
    fn test_join_directory_marker_directory_marker_key() {
        let result = join_directory_marker("prefix", "subdir/");
        assert_eq!(result, "prefix/subdir/");
    }

    #[test]
    fn test_join_directory_marker_collapses_multi_slashes() {
        let result = join_directory_marker("prefix", "my////key");
        assert_eq!(result, "prefix/my/key");
    }

    #[test]
    fn test_join_directory_marker_preserves_directory_marker_suffix() {
        let result = join_directory_marker("prefix/", "subdir/");
        assert_eq!(result, "prefix/subdir/");
    }

    #[test]
    fn test_ensure_directory_marker_adds_suffix() {
        let result = ensure_directory_marker("prefix/subdir").unwrap();
        assert_eq!(result, "prefix/subdir/");
    }

    #[test]
    fn test_ensure_directory_marker_keeps_existing_suffix() {
        let result = ensure_directory_marker("prefix/subdir/").unwrap();
        assert_eq!(result, "prefix/subdir/");
    }

    // ============ apply_path_prefix_constraint tests ============

    #[test]
    fn test_apply_path_prefix_constraint_empty_prefix() {
        let result = apply_path_prefix_constraint("any/key", "");
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_path_prefix_constraint_exact_match() {
        let result = apply_path_prefix_constraint("prefix", "prefix");
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_path_prefix_constraint_key_under_prefix() {
        let result = apply_path_prefix_constraint("prefix/my/key", "prefix");
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_path_prefix_constraint_key_outside_prefix() {
        let result = apply_path_prefix_constraint("other/key", "prefix");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not respect path_prefix")
        );
    }

    #[test]
    fn test_apply_path_prefix_constraint_partial_match_rejected() {
        let result = apply_path_prefix_constraint("prefixother/key", "prefix");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_path_prefix_constraint_nested_prefix() {
        let result = apply_path_prefix_constraint("a/b/c/d/e", "a/b/c");
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_path_prefix_constraint_directory_marker() {
        let result = apply_path_prefix_constraint("prefix/subdir/", "prefix");
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_path_prefix_constraint_accepts_config_style_trailing_slash_prefix() {
        let result = apply_path_prefix_constraint("team-a/demo.png", "team-a/");
        assert!(result.is_ok());
    }

    // ============ strip_path_prefix_for_response tests ============

    #[test]
    fn test_strip_path_prefix_empty_prefix() {
        let result = strip_path_prefix_for_response("my/key", "");
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_strip_path_prefix_matching_prefix() {
        let result = strip_path_prefix_for_response("prefix/my/key", "prefix");
        assert_eq!(result, "my/key");
    }

    #[test]
    fn test_strip_path_prefix_exact_match() {
        let result = strip_path_prefix_for_response("prefix", "prefix");
        assert_eq!(result, "");
    }

    #[test]
    fn test_strip_path_prefix_no_match() {
        let result = strip_path_prefix_for_response("other/key", "prefix");
        assert_eq!(result, "other/key");
    }

    #[test]
    fn test_strip_path_prefix_partial_match_not_stripped() {
        let result = strip_path_prefix_for_response("prefixother/key", "prefix");
        assert_eq!(result, "prefixother/key");
    }

    #[test]
    fn test_strip_path_prefix_nested() {
        let result = strip_path_prefix_for_response("a/b/c/d/e", "a/b/c");
        assert_eq!(result, "d/e");
    }

    #[test]
    fn test_strip_path_prefix_directory_marker() {
        let result = strip_path_prefix_for_response("prefix/subdir/", "prefix");
        assert_eq!(result, "subdir/");
    }

    #[test]
    fn test_strip_path_prefix_handles_config_style_trailing_slash_prefix() {
        let result = strip_path_prefix_for_response("team-a/demo.png", "team-a/");
        assert_eq!(result, "demo.png");
    }
}
