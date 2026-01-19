//! 统一的指标基础设施
//!
//! 提供基于 metrics-exporter-prometheus 的指标收集和导出功能
//! 支持 HTTP 请求指标和自定义业务指标

pub mod business;
pub mod http;
pub mod registry;

pub use business::{ImageMetrics, OcrMetrics};
pub use http::track_http_metrics;
pub use registry::{MetricsRegistry, init_metrics};

// 重新导出 metrics crate 的核心功能，方便业务代码使用
pub use metrics::{Counter, Gauge, Histogram, counter, gauge, histogram};

pub fn increment_counter(name: &'static str) {
    metrics::counter!(name).increment(1);
}
