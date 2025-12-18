//! # pic_recog - 图片识别库
//!
//! 提供多种图片文字识别引擎的统一接口。
//!
//! ## 功能特性
//!
//! - 🔍 多引擎支持：Tesseract OCR（更多引擎即将支持）
//! - 🌍 多语言识别：支持英文、中文、日文等多种语言
//! - ⚙️ 灵活配置：自定义识别参数
//! - 📦 批量处理：支持批量图片识别
//! - 🛡️ 错误处理：完善的错误类型和处理机制
//!
//! ## 快速开始
//!
//! ### 基本使用
//!
//! ```no_run
//! use pic_recog::recognize_image_by_tesseract;
//!
//! let text = recognize_image_by_tesseract("example.png").unwrap();
//! println!("识别结果: {}", text);
//! ```
//!
//! ### 中文识别
//!
//! ```no_run
//! use pic_recog::recognize_chinese_simplified;
//!
//! let text = recognize_chinese_simplified("chinese.png").unwrap();
//! println!("中文识别: {}", text);
//! ```
//!
//! ### 自定义配置
//!
//! ```no_run
//! use pic_recog::{recognize_image_with_config, OcrConfig};
//!
//! let config = OcrConfig::new()
//!     .with_language("chi_sim")
//!     .with_psm(6)
//!     .with_engine_mode(1);
//!
//! let text = recognize_image_with_config("image.png", &config).unwrap();
//! println!("识别结果: {}", text);
//! ```
//!
//! ### 批量处理
//!
//! ```no_run
//! use pic_recog::{recognize_batch, OcrConfig};
//!
//! let images = vec!["image1.png", "image2.png", "image3.png"];
//! let config = OcrConfig::new().with_language("eng");
//! let results = recognize_batch(&images, &config);
//!
//! for (i, result) in results.iter().enumerate() {
//!     match result {
//!         Ok(text) => println!("图片 {}: {}", i + 1, text),
//!         Err(e) => eprintln!("图片 {} 识别失败: {}", i + 1, e),
//!     }
//! }
//! ```
//!
//! ## 模块结构
//!
//! - `config` - 配置类型
//! - `error` - 错误类型定义
//! - `engines` - 不同的识别引擎实现
//!   - `tesseract` - Tesseract OCR 引擎
//! - `utils` - 通用工具函数

// 模块声明
pub mod config;
pub mod engines;
pub mod error;
pub mod utils;

// 重新导出常用类型
pub use config::{OcrConfig, RemoteOcrConfig};
pub use error::ImageRecognitionError;

// ============================================================================
// 公共 API - Tesseract 引擎
// ============================================================================

/// 使用 Tesseract OCR 识别图片中的文字（简单版本）
///
/// 使用默认配置（英文）识别图片中的文字。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
///
/// # 返回
///
/// 提取的文本内容
///
/// # 示例
///
/// ```no_run
/// use pic_recog::recognize_image_by_tesseract;
///
/// let text = recognize_image_by_tesseract("example.png").unwrap();
/// println!("提取的文字: {}", text);
/// ```
///
/// # 错误
///
/// - 文件不存在
/// - 文件格式不支持
/// - Tesseract 未安装或执行失败
pub fn recognize_image_by_tesseract(image_path: &str) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::default();
    engines::tesseract::recognize(image_path, &config)
}

/// 使用自定义配置识别图片中的文字
///
/// 允许指定语言、页面分割模式等高级参数。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
/// * `config` - OCR 配置选项
///
/// # 返回
///
/// 提取的文本内容
///
/// # 示例
///
/// ```no_run
/// use pic_recog::{recognize_image_with_config, OcrConfig};
///
/// let config = OcrConfig::new()
///     .with_language("chi_sim")
///     .with_psm(6);
///
/// let text = recognize_image_with_config("chinese.png", &config).unwrap();
/// println!("提取的文字: {}", text);
/// ```
pub fn recognize_image_with_config(
    image_path: &str,
    config: &OcrConfig,
) -> Result<String, ImageRecognitionError> {
    engines::tesseract::recognize(image_path, config)
}

/// 识别中文简体图片
///
/// 专门用于识别中文简体文字的便捷函数。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
///
/// # 返回
///
/// 提取的文本内容
///
/// # 示例
///
/// ```no_run
/// use pic_recog::recognize_chinese_simplified;
///
/// let text = recognize_chinese_simplified("chinese.png").unwrap();
/// println!("中文识别: {}", text);
/// ```
///
/// # 注意
///
/// 需要安装中文简体语言包：
/// ```bash
/// sudo apt-get install tesseract-ocr-chi-sim
/// ```
pub fn recognize_chinese_simplified(image_path: &str) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::new().with_language("chi_sim");
    engines::tesseract::recognize(image_path, &config)
}

/// 识别中文繁体图片
///
/// 专门用于识别中文繁体文字的便捷函数。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
///
/// # 返回
///
/// 提取的文本内容
pub fn recognize_chinese_traditional(image_path: &str) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::new().with_language("chi_tra");
    engines::tesseract::recognize(image_path, &config)
}

/// 识别日文图片
///
/// 专门用于识别日文文字的便捷函数。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
///
/// # 返回
///
/// 提取的文本内容
pub fn recognize_japanese(image_path: &str) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::new().with_language("jpn");
    engines::tesseract::recognize(image_path, &config)
}

/// 识别多语言图片
///
/// 支持同时识别多种语言的文字。
///
/// # 参数
///
/// * `image_path` - 图片文件路径
/// * `languages` - 语言列表，用 + 分隔 (例如: "eng+chi_sim")
///
/// # 返回
///
/// 提取的文本内容
///
/// # 示例
///
/// ```no_run
/// use pic_recog::recognize_multi_language;
///
/// // 同时识别英文和中文
/// let text = recognize_multi_language("mixed.png", "eng+chi_sim").unwrap();
/// println!("混合语言识别: {}", text);
/// ```
pub fn recognize_multi_language(
    image_path: &str,
    languages: &str,
) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::new().with_language(languages);
    engines::tesseract::recognize(image_path, &config)
}

/// 批量识别图片
///
/// 使用相同配置识别多张图片。
///
/// # 参数
///
/// * `image_paths` - 图片文件路径列表
/// * `config` - OCR 配置选项
///
/// # 返回
///
/// 每个图片的识别结果列表
///
/// # 示例
///
/// ```no_run
/// use pic_recog::{recognize_batch, OcrConfig};
///
/// let images = vec!["img1.png", "img2.png", "img3.png"];
/// let config = OcrConfig::new().with_language("eng");
/// let results = recognize_batch(&images, &config);
///
/// for (i, result) in results.iter().enumerate() {
///     match result {
///         Ok(text) => println!("图片 {}: {}", i + 1, text),
///         Err(e) => eprintln!("图片 {} 失败: {}", i + 1, e),
///     }
/// }
/// ```
pub fn recognize_batch(
    image_paths: &[&str],
    config: &OcrConfig,
) -> Vec<Result<String, ImageRecognitionError>> {
    engines::tesseract::recognize_batch(image_paths, config)
}

/// 使用远程 OCR 服务识别图片
///
/// 在调用远程服务之前会对图片尺寸、体积与格式进行校验。
/// 具体的远程服务端点、鉴权信息等参数通过 `RemoteOcrConfig` 的
/// TOML 配置文件加载。
///
/// # 参数
/// * `image_path` - 图片文件路径
/// * `config` - 远程 OCR 配置
///
/// # 返回
/// 远程 OCR 服务返回的识别文本；若无法解析文本，则返回原始 JSON 响应字符串
pub fn recognize_image_by_remote(
    image_path: &str,
    config: &RemoteOcrConfig,
) -> Result<String, ImageRecognitionError> {
    engines::remote::recognize(image_path, config)
}
