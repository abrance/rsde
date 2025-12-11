//! Tesseract OCR 识别器
//!
//! 实现基于 Tesseract 的图片文字识别功能

use super::utils::get_tessdata_dir;
use crate::config::OcrConfig;
use crate::error::ImageRecognitionError;
use crate::utils::validate_image_path;
use std::process::Command;

/// 使用 Tesseract 识别图片中的文字
///
/// # 参数
/// * `image_path` - 图片文件路径
/// * `config` - OCR 配置选项
///
/// # 返回
/// 提取的文本内容
pub fn recognize(image_path: &str, config: &OcrConfig) -> Result<String, ImageRecognitionError> {
    // 验证图片路径
    validate_image_path(image_path)?;

    // 获取 tessdata 目录
    let tessdata_dir = get_tessdata_dir(config.data_path.as_deref());

    // 构建 tesseract 命令
    let mut cmd = Command::new("tesseract");
    cmd.arg(image_path)
        .arg("stdout")
        .arg("-l")
        .arg(&config.language);

    // 设置页面分割模式
    if let Some(psm) = config.page_segmentation_mode {
        cmd.arg("--psm").arg(psm.to_string());
    }

    // 设置引擎模式
    if let Some(oem) = config.engine_mode {
        cmd.arg("--oem").arg(oem.to_string());
    }

    // 设置 TESSDATA_PREFIX 环境变量
    cmd.env("TESSDATA_PREFIX", tessdata_dir);

    // 执行命令
    let output = cmd.output().map_err(|e| {
        ImageRecognitionError::TesseractError(format!(
            "执行 tesseract 命令失败: {e}. 请确保系统已安装 tesseract-ocr"
        ))
    })?;

    // 检查执行结果
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(ImageRecognitionError::TesseractError(format!(
            "Tesseract 执行失败: {error_msg}"
        )));
    }

    // 解析输出
    let text = String::from_utf8(output.stdout)
        .map_err(|e| ImageRecognitionError::TesseractError(format!("解析输出失败: {e}")))?;

    Ok(text)
}

/// 批量识别图片
///
/// # 参数
/// * `image_paths` - 图片文件路径列表
/// * `config` - OCR 配置选项
///
/// # 返回
/// 每个图片的识别结果
pub fn recognize_batch(
    image_paths: &[&str],
    config: &OcrConfig,
) -> Vec<Result<String, ImageRecognitionError>> {
    image_paths
        .iter()
        .map(|path| recognize(path, config))
        .collect()
}
