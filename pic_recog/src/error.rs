//! 错误类型定义
//!
//! 定义了图片识别过程中可能出现的各种错误类型

use std::fmt;

/// 图片识别错误类型
#[derive(Debug)]
pub enum ImageRecognitionError {
    /// 图片文件不存在
    FileNotFound(String),
    /// Tesseract OCR 错误
    TesseractError(String),
    /// 不支持的图片格式
    UnsupportedFormat(String),
    /// IO 错误
    IoError(std::io::Error),
    /// 其他引擎错误（预留给未来的识别引擎）
    EngineError(String),
}

impl fmt::Display for ImageRecognitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageRecognitionError::FileNotFound(path) => {
                write!(f, "图片文件不存在: {}", path)
            }
            ImageRecognitionError::TesseractError(msg) => {
                write!(f, "Tesseract OCR 错误: {}", msg)
            }
            ImageRecognitionError::UnsupportedFormat(format) => {
                write!(f, "不支持的图片格式: {}", format)
            }
            ImageRecognitionError::IoError(err) => {
                write!(f, "IO 错误: {}", err)
            }
            ImageRecognitionError::EngineError(msg) => {
                write!(f, "识别引擎错误: {}", msg)
            }
        }
    }
}

impl std::error::Error for ImageRecognitionError {}

impl From<std::io::Error> for ImageRecognitionError {
    fn from(err: std::io::Error) -> Self {
        ImageRecognitionError::IoError(err)
    }
}
