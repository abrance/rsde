//! 七牛云对象存储配置

use serde::{Deserialize, Serialize};

const ALLOWED_REGIONS: &[&str] = &["z0", "z1", "z2", "na0", "as0"];

fn collapse_slashes(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_slash = false;

    for ch in input.chars() {
        if ch == '/' {
            if !prev_was_slash {
                result.push(ch);
                prev_was_slash = true;
            }
        } else {
            result.push(ch);
            prev_was_slash = false;
        }
    }

    result
}

fn normalize_path_prefix_segments(input: &str) -> anyhow::Result<Option<String>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let collapsed = collapse_slashes(trimmed.trim_start_matches('/'));
    let mut segments = Vec::new();

    for segment in collapsed.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            return Err(anyhow::anyhow!(
                "object_storage: path_prefix cannot contain .. path segments"
            ));
        }

        segments.push(segment);
    }

    if segments.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!("{}/", segments.join("/"))))
    }
}

fn default_upload_token_ttl_secs() -> u64 {
    3600
}

fn default_private_url_ttl_secs() -> u64 {
    3600
}

fn default_use_https() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObjectStorageConfig {
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub domain: Option<String>,
    pub public_base_url: Option<String>,
    #[serde(default = "default_upload_token_ttl_secs")]
    pub upload_token_ttl_secs: u64,
    #[serde(default = "default_private_url_ttl_secs")]
    pub private_url_ttl_secs: u64,
    #[serde(default = "default_use_https")]
    pub use_https: bool,
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub bucket_is_private: bool,
}

impl ObjectStorageConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.access_key.trim().is_empty() {
            return Err(anyhow::anyhow!("object_storage: access_key is required"));
        }
        if self.secret_key.trim().is_empty() {
            return Err(anyhow::anyhow!("object_storage: secret_key is required"));
        }
        if self.bucket.trim().is_empty() {
            return Err(anyhow::anyhow!("object_storage: bucket is required"));
        }
        if self.region.trim().is_empty() {
            return Err(anyhow::anyhow!("object_storage: region is required"));
        }

        if !ALLOWED_REGIONS.contains(&self.region.as_str()) {
            return Err(anyhow::anyhow!(
                "object_storage: region '{}' is not allowed. Allowed regions: {}",
                self.region,
                ALLOWED_REGIONS.join(", ")
            ));
        }

        if !self.bucket_is_private {
            let has_domain = self.domain.as_ref().map_or(false, |d| !d.trim().is_empty());
            let has_public_base_url = self
                .public_base_url
                .as_ref()
                .map_or(false, |u| !u.trim().is_empty());

            if !has_domain && !has_public_base_url {
                return Err(anyhow::anyhow!(
                    "object_storage: for public bucket, either domain or public_base_url must be configured and non-empty"
                ));
            }
        }

        if let Some(path_prefix) = &self.path_prefix {
            normalize_path_prefix_segments(path_prefix)?;
        }

        Ok(())
    }

    pub fn normalized_path_prefix(&self) -> Option<String> {
        self.path_prefix
            .as_ref()
            .and_then(|p| normalize_path_prefix_segments(p).ok().flatten())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_invalid_region() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "invalid-region".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        assert!(config.validate().is_err());
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("not allowed")
        );
    }

    #[test]
    fn validate_accepts_allowed_regions() {
        for region in ALLOWED_REGIONS {
            let config = ObjectStorageConfig {
                access_key: "ak".to_string(),
                secret_key: "sk".to_string(),
                bucket: "bucket".to_string(),
                region: region.to_string(),
                domain: Some("example.com".to_string()),
                public_base_url: None,
                upload_token_ttl_secs: 3600,
                private_url_ttl_secs: 3600,
                use_https: true,
                path_prefix: None,
                bucket_is_private: false,
            };
            assert!(
                config.validate().is_ok(),
                "region {} should be valid",
                region
            );
        }
    }

    #[test]
    fn validate_rejects_empty_domain_and_public_base_url() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("".to_string()),
            public_base_url: Some("".to_string()),
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_non_empty_domain() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: Some("".to_string()),
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_non_empty_public_base_url() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("".to_string()),
            public_base_url: Some("https://example.com".to_string()),
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_whitespace_only_required_fields() {
        let config = ObjectStorageConfig {
            access_key: "  ".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_allows_private_bucket_without_domain() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: None,
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: None,
            bucket_is_private: true,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn normalized_path_prefix_handles_whitespace_and_slashes() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: Some("  /foo/bar  ".to_string()),
            bucket_is_private: false,
        };
        assert_eq!(
            config.normalized_path_prefix(),
            Some("foo/bar/".to_string())
        );
    }

    #[test]
    fn normalized_path_prefix_returns_none_for_empty() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: Some("  /  ".to_string()),
            bucket_is_private: false,
        };
        assert_eq!(config.normalized_path_prefix(), None);
    }

    #[test]
    fn normalized_path_prefix_collapses_internal_slashes_and_drops_dot_segments() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: Some(" //foo//./bar/// ".to_string()),
            bucket_is_private: false,
        };
        assert_eq!(
            config.normalized_path_prefix(),
            Some("foo/bar/".to_string())
        );
    }

    #[test]
    fn validate_rejects_path_prefix_with_double_dot_segment() {
        let config = ObjectStorageConfig {
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            bucket: "bucket".to_string(),
            region: "z0".to_string(),
            domain: Some("example.com".to_string()),
            public_base_url: None,
            upload_token_ttl_secs: 3600,
            private_url_ttl_secs: 3600,
            use_https: true,
            path_prefix: Some("team-a/../secret".to_string()),
            bucket_is_private: false,
        };
        assert!(config.validate().is_err());
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("path_prefix cannot contain .. path segments")
        );
    }

    #[test]
    fn serde_defaults_for_ttl_and_flags() {
        let toml_str = r#"
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket"
            region = "z0"
            domain = "example.com"
        "#;
        let config: ObjectStorageConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.upload_token_ttl_secs, 3600);
        assert_eq!(config.private_url_ttl_secs, 3600);
        assert_eq!(config.use_https, true);
        assert_eq!(config.bucket_is_private, false);
    }
}
