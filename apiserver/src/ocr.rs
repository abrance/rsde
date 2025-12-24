//! OCR 路由处理模块
//!
//! 提供 OCR 图片识别的 HTTP API

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use config::ocr::RemoteOcrConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// OCR 服务状态
#[derive(Clone)]
pub struct OcrState {
    /// 远程 OCR 配置
    pub remote_config: Arc<RemoteOcrConfig>,
}

/// OCR 单张图片请求体
#[derive(Debug, Deserialize)]
pub struct SinglePicRequest {
    /// 图片路径或 base64 编码
    pub image_path: String,
    /// 语言（可选，默认 eng）
    pub language: Option<String>,
    /// 是否包含坐标信息（可选，默认 false）
    #[serde(default)]
    pub include_position: bool,
}

/// OCR 响应体
#[derive(Debug, Serialize)]
pub struct OcrResponse {
    /// 是否成功
    pub success: bool,
    /// 识别的文本内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 图片路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
}

impl OcrResponse {
    /// 创建成功响应
    pub fn success(text: String, image_path: Option<String>) -> Self {
        Self {
            success: true,
            text: Some(text),
            error: None,
            image_path,
        }
    }

    /// 创建错误响应
    pub fn error(error: String) -> Self {
        Self {
            success: false,
            text: None,
            error: Some(error),
            image_path: None,
        }
    }
}

/// 健康检查
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "ocr-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// 单张图片 OCR 识别 - 使用 remote OCR
///
/// POST /ocr/single_pic
/// Content-Type: application/json
/// Body: { "image_path": "/path/to/image.png", "include_position": true }
async fn single_pic_remote(
    State(state): State<OcrState>,
    Json(payload): Json<SinglePicRequest>,
) -> Result<Json<OcrResponse>, (StatusCode, Json<OcrResponse>)> {
    info!(
        "收到 OCR 请求: image_path={}, include_position={}",
        payload.image_path, payload.include_position
    );

    let image_path = payload.image_path.clone();
    let remote_config = state.remote_config.clone();
    let include_position = payload.include_position;

    // 在阻塞线程池中调用 remote OCR（因为它使用 blocking HTTP client）
    let result = tokio::task::spawn_blocking(move || {
        if include_position {
            pic_recog::recognize_image_by_remote_with_position(&image_path, &remote_config)
        } else {
            pic_recog::recognize_image_by_remote(&image_path, &remote_config)
        }
    })
    .await
    .map_err(|e| {
        error!("OCR 任务执行失败: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OcrResponse::error(format!("OCR 任务执行失败: {e}"))),
        )
    })?;

    match result {
        Ok(text) => {
            info!("OCR 识别成功: {} 字符", text.len());
            Ok(Json(OcrResponse::success(
                text,
                Some(payload.image_path.clone()),
            )))
        }
        Err(e) => {
            error!("OCR 识别失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcrResponse::error(format!("OCR 识别失败: {e}"))),
            ))
        }
    }
}

/// 单张图片 OCR 识别 - 使用本地 Tesseract
///
/// POST /ocr/single_pic_local
/// Content-Type: application/json
/// Body: { "image_path": "/path/to/image.png", "language": "eng" }
async fn single_pic_local(
    Json(payload): Json<SinglePicRequest>,
) -> Result<Json<OcrResponse>, (StatusCode, Json<OcrResponse>)> {
    info!("收到本地 OCR 请求: image_path={}", payload.image_path);

    let image_path = payload.image_path.clone();
    let language = payload.language.as_deref().unwrap_or("eng").to_string();

    // 在阻塞线程池中调用本地 Tesseract OCR（因为它执行外部命令）
    let result = tokio::task::spawn_blocking(move || {
        pic_recog::recognize_image_with_config(
            &image_path,
            &pic_recog::OcrConfig::new().with_language(language),
        )
    })
    .await
    .map_err(|e| {
        error!("OCR 任务执行失败: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OcrResponse::error(format!("OCR 任务执行失败: {e}"))),
        )
    })?;

    match result {
        Ok(text) => {
            info!("本地 OCR 识别成功: {} 字符", text.len());
            Ok(Json(OcrResponse::success(
                text,
                Some(payload.image_path.clone()),
            )))
        }
        Err(e) => {
            error!("本地 OCR 识别失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcrResponse::error(format!("本地 OCR 识别失败: {e}"))),
            ))
        }
    }
}

/// 创建 OCR 路由
pub fn create_routes(remote_config: RemoteOcrConfig) -> Router {
    let state = OcrState {
        remote_config: Arc::new(remote_config),
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/single_pic", post(single_pic_remote))
        .route("/single_pic_local", post(single_pic_local))
        .with_state(state)
}
