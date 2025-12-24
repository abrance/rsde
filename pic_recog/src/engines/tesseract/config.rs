//! Tesseract 专用配置
//!
//! 提供 Tesseract OCR 引擎的配置选项

use config::ocr::OcrConfig;

/// Tesseract 配置扩展
pub struct TesseractConfig {
    /// 基础配置
    pub base: OcrConfig,
}

impl TesseractConfig {
    /// 从通用 OcrConfig 创建
    pub fn from_ocr_config(config: &OcrConfig) -> Self {
        TesseractConfig {
            base: config.clone(),
        }
    }

    /// 创建新的 Tesseract 配置
    pub fn new() -> Self {
        TesseractConfig {
            base: OcrConfig::default(),
        }
    }

    /// 设置语言
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.base.language = language.into();
        self
    }

    /// 设置数据路径
    pub fn with_data_path(mut self, path: impl Into<String>) -> Self {
        self.base.data_path = Some(path.into());
        self
    }

    /// 设置页面分割模式
    pub fn with_psm(mut self, mode: i32) -> Self {
        self.base.page_segmentation_mode = Some(mode);
        self
    }

    /// 设置引擎模式
    pub fn with_engine_mode(mut self, mode: i32) -> Self {
        self.base.engine_mode = Some(mode);
        self
    }

    /// 获取基础配置的引用
    pub fn base_config(&self) -> &OcrConfig {
        &self.base
    }
}

impl Default for TesseractConfig {
    fn default() -> Self {
        Self::new()
    }
}
