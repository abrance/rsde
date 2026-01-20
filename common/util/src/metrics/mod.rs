pub mod business;
#[cfg(feature = "metrics")]
pub mod http;
pub mod registry;

pub use business::{ImageMetrics, OcrMetrics};
#[cfg(feature = "metrics")]
pub use http::track_http_metrics;
pub use registry::{MetricsRegistry, init_metrics};

pub use metrics::{Counter, Gauge, Histogram, counter, gauge, histogram};

pub fn increment_counter(name: &'static str) {
    metrics::counter!(name).increment(1);
}
