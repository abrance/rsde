//! 通用工具函数
//!
//! 提供跨引擎使用的工具函数

use crate::error::ImageRecognitionError;
use std::path::Path;

/// 支持的图片格式
const SUPPORTED_FORMATS: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "webp"];

/// 验证图片文件是否存在且格式支持
///
/// # 参数
/// * `image_path` - 图片文件路径
///
/// # 返回
/// * `Ok(())` - 文件存在且格式支持
/// * `Err(ImageRecognitionError)` - 文件不存在或格式不支持
pub fn validate_image_path(image_path: &str) -> Result<(), ImageRecognitionError> {
    let path = Path::new(image_path);

    // 检查文件是否存在
    if !path.exists() {
        return Err(ImageRecognitionError::FileNotFound(image_path.to_string()));
    }

    // 检查文件扩展名
    if let Some(extension) = path.extension() {
        let ext = extension.to_str().unwrap_or("").to_lowercase();
        if !SUPPORTED_FORMATS.contains(&ext.as_str()) {
            return Err(ImageRecognitionError::UnsupportedFormat(ext));
        }
    } else {
        return Err(ImageRecognitionError::UnsupportedFormat(
            "无扩展名".to_string(),
        ));
    }

    Ok(())
}

/// 获取支持的图片格式列表
pub fn supported_formats() -> &'static [&'static str] {
    SUPPORTED_FORMATS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_formats() {
        let formats = supported_formats();
        assert!(formats.contains(&"png"));
        assert!(formats.contains(&"jpg"));
        assert!(formats.contains(&"jpeg"));
    }
}
