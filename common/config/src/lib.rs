//! 统一配置管理模块
//!
//! 整合 pic_recog, rsync, rc 等所有服务的配置定义

pub mod anybox;
pub mod apiserver;
pub mod image_host;
pub mod mysql;
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
}

impl ConfigLoader for GlobalConfig {
    fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
