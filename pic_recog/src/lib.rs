//! # pic_recog - 图片识别库
//!
//! 提供多种图片文字识别引擎的统一接口。
//!
//! ## 功能特性
//!
//! - 🔍 多引擎支持：Remote OCR（更多引擎即将支持）
//! - 🌍 多语言识别：支持英文、中文、日文等多种语言
//! - ⚙️ 灵活配置：自定义识别参数
//! - 🛡️ 错误处理：完善的错误类型和处理机制
//!
//! ## 快速开始
//!
//! ### 基本使用 - Remote OCR
//!
//! ```no_run
//! use pic_recog::{recognize_image_by_remote, RemoteOcrConfig};
//!
//! // 从配置文件加载
//! let config = RemoteOcrConfig::from_file("config.toml").unwrap();
//! let text = recognize_image_by_remote("example.png", &config).unwrap();
//! println!("识别结果: {}", text);
//! ```
//!
//! ### 获取坐标信息
//!
//! ```no_run
//! use pic_recog::{recognize_image_by_remote_with_position, RemoteOcrConfig};
//!
//! // 从配置文件加载
//! let config = RemoteOcrConfig::from_file("config.toml").unwrap();
//! let result = recognize_image_by_remote_with_position("image.png", &config).unwrap();
//! println!("识别结果（含坐标）: {}", result);
//! ```
//!
//! ## 模块结构
//!
//! - `config` - 配置类型
//! - `error` - 错误类型定义
//! - `engines` - 不同的识别引擎实现
//!   - `remote` - Remote OCR 引擎
//! - `utils` - 通用工具函数

// 模块声明
pub mod engines;
pub mod error;
pub mod utils;

// 重新导出常用类型
pub use config::ocr::RemoteOcrConfig;
pub use error::ImageRecognitionError;

// ============================================================================
// 公共 API - Remote OCR 引擎
// ============================================================================

/// 使用远程 OCR 服务识别图片（仅文本）
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
/// 远程 OCR 服务返回的识别文本（不包含坐标信息）
pub fn recognize_image_by_remote(
    image_path: &str,
    config: &RemoteOcrConfig,
) -> Result<String, ImageRecognitionError> {
    engines::remote::recognize(image_path, config, false)
}

/// 使用远程 OCR 服务识别图片（包含完整坐标信息）
///
/// 在调用远程服务之前会对图片尺寸、体积与格式进行校验。
/// 返回包含文本坐标等位置信息的完整 JSON 结果。
///
/// # 参数
/// * `image_path` - 图片文件路径
/// * `config` - 远程 OCR 配置
///
/// # 返回
/// 包含坐标信息的完整 JSON 结果字符串
pub fn recognize_image_by_remote_with_position(
    image_path: &str,
    config: &RemoteOcrConfig,
) -> Result<String, ImageRecognitionError> {
    engines::remote::recognize(image_path, config, true)
}
