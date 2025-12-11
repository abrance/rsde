//! Tesseract OCR 引擎
//!
//! 基于 Tesseract OCR 的图片文字识别实现

pub mod config;
pub mod recognizer;
pub mod utils;

// 重新导出常用类型和函数
pub use config::TesseractConfig;
pub use recognizer::{recognize, recognize_batch};
