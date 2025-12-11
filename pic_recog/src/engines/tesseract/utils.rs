//! Tesseract 工具函数
//!
//! 提供 Tesseract 引擎专用的工具函数

use std::path::PathBuf;

const DEFAULT_TESSDATA_DIR: &str = "./train_data";

/// 获取默认的 tessdata 目录
pub fn get_default_tessdata_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| String::from(DEFAULT_TESSDATA_DIR));
        PathBuf::from(home_dir)
            .join("Library")
            .join("Application Support")
            .join("tesseract-rs")
            .join("tessdata")
    } else if cfg!(target_os = "linux") {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| String::from(DEFAULT_TESSDATA_DIR));
        PathBuf::from(home_dir)
            .join(".tesseract-rs")
            .join("tessdata")
    } else if cfg!(target_os = "windows") {
        let app_data =
            std::env::var("APPDATA").unwrap_or_else(|_| String::from(DEFAULT_TESSDATA_DIR));
        PathBuf::from(app_data)
            .join("tesseract-rs")
            .join("tessdata")
    } else {
        panic!("unsupported os")
    }
}

/// 获取 tessdata 目录（优先使用环境变量，否则使用默认路径）
///
/// # 参数
/// * `custom_path` - 自定义路径（可选）
///
/// # 返回
/// tessdata 目录路径
pub fn get_tessdata_dir(custom_path: Option<&str>) -> PathBuf {
    // 如果提供了自定义路径，使用它
    if let Some(path) = custom_path {
        return PathBuf::from(path);
    }

    // 否则检查环境变量
    match std::env::var("TESSDATA_PREFIX") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => get_default_tessdata_dir(),
    }
}
