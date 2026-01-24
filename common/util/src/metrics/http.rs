use axum::{body::HttpBody, extract::Request, middleware::Next, response::Response};
use metrics::{counter, gauge, histogram};
use std::time::Instant;

pub async fn track_http_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let request_size = if let Some(content_length) = request.headers().get("content-length") {
        content_length.to_str().unwrap_or("0").parse().unwrap_or(0)
    } else {
        0
    };

    gauge!("http_active_connections").increment(1.0);

    let response = next.run(request).await;

    gauge!("http_active_connections").decrement(1.0);

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let response_size = response
        .size_hint()
        .upper()
        .unwrap_or(response.size_hint().lower());

    let labels = [
        ("method", method.clone()),
        ("path", path.clone()),
        ("status", status.clone()),
    ];
    counter!("http_requests_total", &labels).increment(1);

    let labels = [
        ("method", method.clone()),
        ("path", path.clone()),
        ("status", status.clone()),
    ];
    histogram!("http_requests_duration_seconds", &labels).record(duration);

    if request_size > 0 {
        let labels = [("method", method.clone()), ("path", path.clone())];
        histogram!("http_request_size_bytes", &labels).record(request_size as f64);
    }

    if response_size > 0 {
        let labels = [("method", method), ("path", path), ("status", status)];
        histogram!("http_response_size_bytes", &labels).record(response_size as f64);
    }

    response
}
