//! 通用工具函数
//!
//! 提供跨引擎使用的工具函数

use crate::error::ImageRecognitionError;
use image::image_dimensions;
use std::fs;
use std::path::Path;

/// 支持的图片格式
const SUPPORTED_FORMATS: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "webp"];

/// 远程 OCR 支持的图片及文档格式
const REMOTE_SUPPORTED_FORMATS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "tiff", "tif", "webp", "pdf",
];

const MAX_REMOTE_PAYLOAD_BYTES: u64 = 10 * 1024 * 1024; // 10MB
const MIN_DIMENSION: u32 = 16;
const MAX_DIMENSION: u32 = 8192;
const MAX_ASPECT_RATIO: f32 = 50.0;

/// 远程 OCR 通过校验的图片负载
pub struct RemoteImagePayload {
    pub bytes: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: String,
}

impl RemoteImagePayload {
    /// 获取与文件扩展名对应的 MIME 类型
    pub fn mime_type(&self) -> Result<&'static str, ImageRecognitionError> {
        match self.format.as_str() {
            "png" => Ok("image/png"),
            "jpg" | "jpeg" => Ok("image/jpeg"),
            "bmp" => Ok("image/bmp"),
            "gif" => Ok("image/gif"),
            "tiff" | "tif" => Ok("image/tiff"),
            "webp" => Ok("image/webp"),
            "pdf" => Ok("application/pdf"),
            other => Err(ImageRecognitionError::UnsupportedFormat(other.to_string())),
        }
    }
}

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

/// 加载并校验远程 OCR 图片输入
pub fn load_and_validate_remote_image(
    image_path: &str,
) -> Result<RemoteImagePayload, ImageRecognitionError> {
    let path = Path::new(image_path);

    if !path.exists() {
        return Err(ImageRecognitionError::FileNotFound(image_path.to_string()));
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| ImageRecognitionError::UnsupportedFormat("无扩展名".to_string()))?;

    if !REMOTE_SUPPORTED_FORMATS.contains(&ext.as_str()) {
        return Err(ImageRecognitionError::UnsupportedFormat(ext));
    }

    let bytes = fs::read(path)?;

    if bytes.len() as u64 > MAX_REMOTE_PAYLOAD_BYTES {
        return Err(ImageRecognitionError::ValidationError(format!(
            "图片体积超出限制 (最大 10MB)，当前大小: {:.2}MB",
            bytes.len() as f64 / 1_048_576.0
        )));
    }

    if ext != "pdf" {
        let (width, height) = image_dimensions(path).map_err(|err| {
            ImageRecognitionError::ValidationError(format!("读取图片尺寸失败: {err}"))
        })?;

        validate_dimensions(width, height)?;

        return Ok(RemoteImagePayload {
            bytes,
            width: Some(width),
            height: Some(height),
            format: ext,
        });
    }

    Ok(RemoteImagePayload {
        bytes,
        width: None,
        height: None,
        format: ext,
    })
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ImageRecognitionError> {
    if width < MIN_DIMENSION || height < MIN_DIMENSION {
        return Err(ImageRecognitionError::ValidationError(format!(
            "图片尺寸过小: {width}x{height}，要求最小边 >= {MIN_DIMENSION} 像素"
        )));
    }

    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(ImageRecognitionError::ValidationError(format!(
            "图片尺寸过大: {width}x{height}，要求最大边 <= {MAX_DIMENSION} 像素"
        )));
    }

    if width == 0 || height == 0 {
        return Err(ImageRecognitionError::ValidationError(
            "图片尺寸非法: 宽或高为 0".to_string(),
        ));
    }

    let (longer, shorter) = if width > height {
        (width as f32, height as f32)
    } else {
        (height as f32, width as f32)
    };

    let ratio = longer / shorter.max(1.0);
    if ratio >= MAX_ASPECT_RATIO {
        return Err(ImageRecognitionError::ValidationError(format!(
            "图片长宽比过大: {:.2}，要求小于 {MAX_ASPECT_RATIO}",
            ratio
        )));
    }

    Ok(())
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

    #[test]
    fn test_validate_dimensions_ok() {
        assert!(validate_dimensions(1024, 768).is_ok());
    }

    #[test]
    fn test_validate_dimensions_rejects_small() {
        assert!(validate_dimensions(10, 600).is_err());
    }

    #[test]
    fn test_validate_dimensions_rejects_large() {
        assert!(validate_dimensions(9000, 800).is_err());
    }

    #[test]
    fn test_validate_dimensions_rejects_ratio() {
        assert!(validate_dimensions(8000, 100).is_err());
    }
}
