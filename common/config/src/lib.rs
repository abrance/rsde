//! 统一配置管理模块
//!
//! 整合 pic_recog, rsync, rc 等所有服务的配置定义

pub mod anybox;
pub mod apiserver;
pub mod datalink_engine;
pub mod image_host;
pub mod mysql;
pub mod nodemanage;
pub mod object_storage;
pub mod ocr;
pub mod prompt;
pub mod redis;
pub mod rsync;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 通用配置加载trait
pub trait ConfigLoader: Sized {
    /// 从TOML文件加载配置
    fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self>;
}

/// 全局项目配置（根配置文件）
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    /// API Server 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apiserver: Option<apiserver::ApiServerConfig>,

    /// 远程 OCR 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ocr: Option<ocr::RemoteOcrConfig>,

    /// Rsync 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rsync: Option<rsync::RsyncConfig>,

    /// 图床配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_hosting: Option<image_host::ImageHostingConfig>,

    /// Redis 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redis: Option<redis::RedisConfig>,

    /// Anybox 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anybox: Option<anybox::AnyboxConfig>,

    /// Prompt 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<prompt::PromptConfig>,

    /// 对象存储配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_storage: Option<object_storage::ObjectStorageConfig>,

    /// DataLink Engine 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datalink_engine: Option<datalink_engine::DataLinkEngineConfig>,

    /// NodeManage 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodemanage: Option<nodemanage::NodeManageConfig>,
}

impl ConfigLoader for GlobalConfig {
    fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}

impl GlobalConfig {
    pub fn validate(&mut self) -> anyhow::Result<()> {
        if let Some(ref storage) = self.object_storage {
            storage.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_storage_config_can_be_deserialized() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
            bucket_is_private = true
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let storage = cfg.object_storage.expect("object_storage should exist");
        assert_eq!(storage.bucket, "bucket-a");
        assert_eq!(storage.region, "z0");
        assert!(storage.bucket_is_private);
    }

    #[test]
    fn object_storage_config_fails_on_load_with_invalid_region() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "invalid-region"
            domain = "cdn.example.com"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn object_storage_config_fails_on_load_with_empty_required_fields() {
        let raw = r#"
            [object_storage]
            access_key = ""
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_err());
    }

    #[test]
    fn object_storage_config_fails_on_load_public_bucket_without_domain() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            bucket_is_private = false
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_err());
    }

    #[test]
    fn object_storage_config_fails_on_load_with_empty_domain_strings() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = ""
            public_base_url = ""
            bucket_is_private = false
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_err());
    }

    #[test]
    fn object_storage_normalizes_path_prefix() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
            path_prefix = "/team-a"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let storage = cfg.object_storage.unwrap();
        assert!(storage.validate().is_ok());
        assert_eq!(
            storage.normalized_path_prefix(),
            Some("team-a/".to_string())
        );
    }

    #[test]
    fn object_storage_normalizes_path_prefix_like_runtime_rules() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
            path_prefix = "//team-a//./images///"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let storage = cfg.object_storage.unwrap();
        assert!(storage.validate().is_ok());
        assert_eq!(
            storage.normalized_path_prefix(),
            Some("team-a/images/".to_string())
        );
    }

    #[test]
    fn object_storage_config_rejects_invalid_path_prefix_segments() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
            path_prefix = "team-a/../secret"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_err());
    }

    #[test]
    fn object_storage_config_passes_validation_on_load_with_valid_config() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "z0"
            domain = "cdn.example.com"
            bucket_is_private = false
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.object_storage.as_ref().unwrap().validate();
        assert!(result.is_ok());
    }

    #[test]
    fn global_config_from_file_validates_object_storage() {
        let raw = r#"
            [object_storage]
            access_key = "ak"
            secret_key = "sk"
            bucket = "bucket-a"
            region = "invalid-region"
            domain = "cdn.example.com"
        "#;

        let mut cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let result = cfg.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_example_can_be_loaded() {
        let config = GlobalConfig::from_file("../../config.example.toml").unwrap();

        assert!(config.apiserver.is_some());
        assert!(config.remote_ocr.is_some());
        assert!(config.rsync.is_some());
        assert!(config.object_storage.is_some());
        assert!(config.datalink_engine.is_some());

        let nodemanage = config.nodemanage.unwrap();
        assert_eq!(nodemanage.table_prefix, "node_");
        assert!(nodemanage.rsagent_package_url.is_some());
        assert!(nodemanage.mysql.is_some());
        assert_eq!(nodemanage.install_root, "/opt/rsagent");
        assert_eq!(
            nodemanage.register_callback_url,
            "http://127.0.0.1:3000/api/nodes/agent/register"
        );
        assert_eq!(nodemanage.install_plugins.len(), 2);
        assert_eq!(nodemanage.install_plugins[0].name, "metrics");
        assert_eq!(nodemanage.install_plugins[1].name, "shell");
        assert_eq!(nodemanage.register_wait_timeout_secs, 30);
    }

    #[test]
    fn nodemanage_config_defaults_install_contract() {
        let raw = r#"
            [nodemanage]
            rsagent_package_url = "https://example.com/rsagent.tar.gz"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let nodemanage = cfg.nodemanage.unwrap();

        assert_eq!(nodemanage.install_root, "/opt/rsagent");
        assert_eq!(
            nodemanage.register_callback_url,
            "http://127.0.0.1:3000/api/nodes/agent/register"
        );
        assert!(nodemanage.install_plugins.is_empty());
        assert_eq!(nodemanage.register_wait_timeout_secs, 30);
    }

    #[test]
    fn image_hosting_config_can_be_deserialized() {
        let raw = r#"
            [image_hosting]
            storage_dir = "/tmp/uploads"
            cleanup_interval_secs = 600
            file_expire_secs = 1800
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let hosting = cfg.image_hosting.expect("image_hosting should exist");
        assert_eq!(hosting.storage_dir, "/tmp/uploads");
        assert_eq!(hosting.cleanup_interval_secs, 600);
        assert_eq!(hosting.file_expire_secs, 1800);
    }

    #[test]
    fn image_hosting_config_uses_defaults_for_optional_fields() {
        let raw = r#"
            [image_hosting]
            storage_dir = "/tmp/uploads"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        let hosting = cfg.image_hosting.unwrap();
        assert_eq!(hosting.storage_dir, "/tmp/uploads");
        assert_eq!(hosting.cleanup_interval_secs, 3600);
        assert_eq!(hosting.file_expire_secs, 3600);
    }

    #[test]
    fn image_hosting_config_allows_missing_section() {
        let raw = r#"
            [apiserver]
            listen_address = "0.0.0.0:3000"
        "#;

        let cfg: GlobalConfig = toml::from_str(raw).unwrap();
        assert!(cfg.image_hosting.is_none());
    }

    #[test]
    fn image_hosting_config_default_impl_matches_field_defaults() {
        let hosting = image_host::ImageHostingConfig::default();
        assert_eq!(hosting.storage_dir, "");
        assert_eq!(hosting.cleanup_interval_secs, 3600);
        assert_eq!(hosting.file_expire_secs, 3600);
    }
}
