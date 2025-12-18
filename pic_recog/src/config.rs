//! 通用配置类型
//!
//! 定义了图片识别的通用配置选项

use crate::error::ImageRecognitionError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// OCR 配置选项
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// 语言代码 (例如: "eng", "chi_sim", "chi_tra", "jpn")
    pub language: String,
    /// 数据路径 (可选，用于指定模型数据目录)
    pub data_path: Option<String>,
    /// 页面分割模式 (PSM) - Tesseract 专用
    pub page_segmentation_mode: Option<i32>,
    /// OCR 引擎模式 - Tesseract 专用
    pub engine_mode: Option<i32>,
}

impl Default for OcrConfig {
    fn default() -> Self {
        OcrConfig {
            language: "eng".to_string(),
            data_path: None,
            page_segmentation_mode: None,
            engine_mode: None,
        }
    }
}

impl OcrConfig {
    /// 创建新的 OCR 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置语言
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// 设置数据路径
    pub fn with_data_path(mut self, path: impl Into<String>) -> Self {
        self.data_path = Some(path.into());
        self
    }

    /// 设置页面分割模式 (Tesseract 专用)
    ///
    /// PSM 模式:
    /// - 0 = 仅定向和脚本检测 (OSD)
    /// - 1 = 带 OSD 的自动页面分割
    /// - 3 = 全自动页面分割，但没有 OSD（默认）
    /// - 6 = 假设单个统一的文本块
    /// - 7 = 将图像视为单个文本行
    /// - 8 = 将图像视为单个单词
    /// - 11 = 稀疏文本。尽可能多地找到文本，没有特定的顺序
    pub fn with_psm(mut self, mode: i32) -> Self {
        self.page_segmentation_mode = Some(mode);
        self
    }

    /// 设置 OCR 引擎模式 (Tesseract 专用)
    ///
    /// OEM 模式:
    /// - 0 = 仅传统引擎
    /// - 1 = 仅神经网络 LSTM 引擎
    /// - 2 = 传统 + LSTM 引擎
    /// - 3 = 默认，基于可用内容
    pub fn with_engine_mode(mut self, mode: i32) -> Self {
        self.engine_mode = Some(mode);
        self
    }
}

/// 远程 OCR 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RemoteOcrConfig {
    /// 获取调用凭证之前的接口地址
    pub perm_url: String,
    /// 启动 OCR 任务的接口地址
    pub start_url: String,
    /// 查询 OCR 结果的接口地址
    pub status_url: String,
    /// 固定授权 token（抓包获取）
    pub auth_token: String,
    /// 固定授权 uuid（抓包获取）
    pub auth_uuid: String,
    /// 会话 cookie（抓包获取）
    pub auth_cookie: String,
    /// 请求来源 origin & referer 头
    #[serde(default = "default_origin")]
    pub origin: String,
    /// perm 接口所需的模式参数
    #[serde(default = "default_mode")]
    pub mode: String,
    /// 单次 HTTP 请求的超时时间（秒）
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// 轮询间隔（毫秒）
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
    /// 轮询最大次数
    #[serde(default = "default_poll_max_attempts")]
    pub poll_max_attempts: u32,
    /// 发起轮询前的初始等待时间（毫秒）
    #[serde(default)]
    pub poll_initial_delay_ms: u64,
    /// 是否忽略 TLS 证书校验（默认 false）
    #[serde(default)]
    pub accept_invalid_certs: bool,
}

impl RemoteOcrConfig {
    /// 从 TOML 配置加载远程 OCR 设置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ImageRecognitionError> {
        let content = fs::read_to_string(&path).map_err(|err| {
            ImageRecognitionError::ConfigError(format!("读取远程 OCR 配置失败: {err}"))
        })?;

        toml::from_str(&content).map_err(|err| {
            ImageRecognitionError::ConfigError(format!("解析远程 OCR 配置失败: {err}"))
        })
    }

    /// 请求超时时间
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs.max(1))
    }

    /// 轮询间隔
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms.max(50))
    }

    /// 初始等待时间
    pub fn poll_initial_delay(&self) -> Duration {
        Duration::from_millis(self.poll_initial_delay_ms)
    }

    /// 判断配置是否仍为占位符（未填入真实凭证）
    pub fn is_placeholder(&self) -> bool {
        self.auth_token.contains("changeme")
            || self.auth_uuid.contains("changeme")
            || self.auth_cookie.contains("changeme")
    }
}

fn default_origin() -> String {
    "https://web.xxxxapp.com".to_string()
}

fn default_mode() -> String {
    "single".to_string()
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_poll_interval_ms() -> u64 {
    500
}

fn default_poll_max_attempts() -> u32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_ocr_config_from_file() {
        let test_config_content = r#"
perm_url = "https://example.com/perm"
start_url = "https://example.com/start"
status_url = "https://example.com/status"
auth_token = "changeme"
auth_uuid = "changeme"
auth_cookie = "changeme"
origin = "https://web.xxxxapp.com"
mode = "single"
"#;
        let tmp_path = std::env::temp_dir().join("remote_ocr_example.toml");
        fs::write(&tmp_path, test_config_content).unwrap();
        let config = RemoteOcrConfig::from_file(&tmp_path);
        assert!(config.is_ok());
        let cfg = config.unwrap();
        assert_eq!(cfg.mode, "single");
        assert_eq!(cfg.timeout_secs, 30);
        fs::remove_file(tmp_path).ok();
    }

    #[test]
    fn test_remote_ocr_config_is_placeholder() {
        let placeholder_config = RemoteOcrConfig {
            perm_url: "https://example.com/perm".to_string(),
            start_url: "https://example.com/start".to_string(),
            status_url: "https://example.com/status".to_string(),
            auth_token: "changeme".to_string(),
            auth_uuid: "changeme".to_string(),
            auth_cookie: "changeme".to_string(),
            origin: default_origin(),
            mode: default_mode(),
            timeout_secs: default_timeout_secs(),
            poll_interval_ms: default_poll_interval_ms(),
            poll_max_attempts: default_poll_max_attempts(),
            poll_initial_delay_ms: 0,
            accept_invalid_certs: false,
        };
        assert!(placeholder_config.is_placeholder());

        let valid_config = RemoteOcrConfig {
            auth_token: "valid_token".to_string(),
            auth_uuid: "valid_uuid".to_string(),
            auth_cookie: "valid_cookie".to_string(),
            ..placeholder_config
        };
        assert!(!valid_config.is_placeholder());
    }
}
