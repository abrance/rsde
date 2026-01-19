use axum::{extract::Request, middleware::Next, response::Response};
use metrics::{counter, histogram};
use std::time::Instant;

/// HTTP 请求指标跟踪中间件
///
/// 自动记录 HTTP 请求的计数、持续时间和状态码
pub async fn track_http_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // HTTP 请求总数
    let labels = [
        ("method", method.clone()),
        ("path", path.clone()),
        ("status", status.clone()),
    ];
    counter!("http_requests_total", &labels).increment(1);

    // HTTP 请求持续时间
    let labels = [("method", method), ("path", path), ("status", status)];
    histogram!("http_requests_duration_seconds", &labels).record(duration);

    response
}
