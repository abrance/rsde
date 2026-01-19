use metrics::{counter, gauge, histogram};

/// 图片相关指标的封装
///
/// 提供高层次的 API 来记录图片上传、清理等操作的指标
pub struct ImageMetrics;

impl ImageMetrics {
    /// 记录图片上传成功
    pub fn record_upload_success(size_bytes: u64) {
        counter!("image_upload_total").increment(1);
        histogram!("image_upload_size_bytes").record(size_bytes as f64);
    }

    /// 记录图片上传失败
    pub fn record_upload_error() {
        counter!("image_upload_errors_total").increment(1);
    }

    /// 记录图片清理操作
    pub fn record_cleanup(deleted_files: u64, freed_bytes: u64) {
        counter!("image_cleanup_deleted_total").increment(deleted_files);
        counter!("image_cleanup_freed_bytes").increment(freed_bytes);
    }

    /// 记录存储空间使用情况
    pub fn record_storage_usage(bytes: u64) {
        gauge!("image_storage_usage_bytes").set(bytes as f64);
    }
}
