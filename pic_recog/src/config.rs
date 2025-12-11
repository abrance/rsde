//! 通用配置类型
//!
//! 定义了图片识别的通用配置选项

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
