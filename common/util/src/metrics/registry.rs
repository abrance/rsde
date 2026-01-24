use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

/// 全局指标注册器
///
/// 负责初始化 metrics recorder 和管理 Prometheus handle
pub struct MetricsRegistry {
    handle: PrometheusHandle,
}

impl MetricsRegistry {
    /// 获取全局指标注册器实例
    ///
    /// 首次调用时会初始化 metrics recorder
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<MetricsRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let builder = PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("http_requests_duration_seconds".to_string()),
                    &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0],
                )
                .unwrap()
                .set_buckets_for_metric(
                    Matcher::Full("http_request_size_bytes".to_string()),
                    &[
                        1024.0,
                        10240.0,
                        102400.0,
                        1048576.0,
                        10485760.0,
                        104857600.0,
                    ],
                )
                .unwrap()
                .set_buckets_for_metric(
                    Matcher::Full("http_response_size_bytes".to_string()),
                    &[
                        1024.0,
                        10240.0,
                        102400.0,
                        1048576.0,
                        10485760.0,
                        104857600.0,
                    ],
                )
                .unwrap()
                .set_buckets_for_metric(
                    Matcher::Full("image_upload_size_bytes".to_string()),
                    &[
                        1024.0, 10240.0, 102400.0, 512000.0, 1048576.0, 5242880.0, 10485760.0,
                        52428800.0,
                    ],
                )
                .unwrap();

            let handle = builder
                .install_recorder()
                .expect("Failed to install Prometheus recorder");

            MetricsRegistry { handle }
        })
    }

    /// 获取 Prometheus handle 用于指标收集
    pub fn handle(&self) -> &PrometheusHandle {
        &self.handle
    }
}

/// 初始化指标系统
///
/// 这个函数应该在应用启动时调用
pub fn init_metrics() -> &'static MetricsRegistry {
    MetricsRegistry::global()
}
