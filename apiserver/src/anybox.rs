use anybox::{PaginationParams, RedisConfig, TextBox, TextBoxManager};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Anybox 服务状态
#[derive(Clone)]
pub struct AnyboxState {
    manager: Arc<Mutex<TextBoxManager>>,
}

impl AnyboxState {
    pub async fn new(config: config::anybox::AnyboxConfig) -> anyhow::Result<Self> {
        let redis_config = RedisConfig::new(config.redis_url).with_prefix(config.key_prefix);

        let manager = TextBoxManager::new(redis_config).await?;

        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }
}

/// 创建 TextBox 请求
#[derive(Debug, Deserialize)]
pub struct CreateTextBoxRequest {
    pub author: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_hours: Option<u64>,
}

/// TextBox 响应
#[derive(Debug, Serialize)]
pub struct TextBoxResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<TextBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 列表响应
#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<anybox::PaginatedResult<TextBox>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 创建 TextBox
async fn create_textbox(
    State(state): State<AnyboxState>,
    Json(req): Json<CreateTextBoxRequest>,
) -> Result<Json<TextBoxResponse>, (StatusCode, Json<TextBoxResponse>)> {
    info!("创建 TextBox: author={}", req.author);

    let mut text_box = TextBox::new(req.author, req.content);

    if let Some(title) = req.title {
        text_box = text_box.with_title(title);
    }

    if let Some(format_str) = req.format {
        if let Some(format) = anybox::TextFormat::from_str(&format_str) {
            text_box = text_box.with_format(format);
        }
    }

    if let Some(language) = req.language {
        text_box = text_box.with_language(language);
    }

    if !req.tags.is_empty() {
        text_box = text_box.with_tags(req.tags);
    }

    if let Some(is_public) = req.is_public {
        text_box = text_box.with_public(is_public);
    }

    if let Some(expire_hours) = req.expire_hours {
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(expire_hours as i64);
        text_box = text_box.with_expires_at(expires_at);
    }

    let mut manager = state.manager.lock().await;
    match manager.create(text_box).await {
        Ok(created) => Ok(Json(TextBoxResponse {
            success: true,
            data: Some(created),
            error: None,
        })),
        Err(e) => {
            error!("创建 TextBox 失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TextBoxResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// 获取 TextBox
async fn get_textbox(
    State(state): State<AnyboxState>,
    Path(id): Path<String>,
) -> Result<Json<TextBoxResponse>, (StatusCode, Json<TextBoxResponse>)> {
    info!("获取 TextBox: id={}", id);

    let mut manager = state.manager.lock().await;
    match manager.get(&id).await {
        Ok(Some(text_box)) => Ok(Json(TextBoxResponse {
            success: true,
            data: Some(text_box),
            error: None,
        })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(TextBoxResponse {
                success: false,
                data: None,
                error: Some("TextBox 不存在".to_string()),
            }),
        )),
        Err(e) => {
            error!("获取 TextBox 失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TextBoxResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// 列出 TextBox
async fn list_textboxes(
    State(state): State<AnyboxState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ListResponse>, (StatusCode, Json<ListResponse>)> {
    info!(
        "列出 TextBox: page={}, page_size={}",
        params.page, params.page_size
    );

    let mut manager = state.manager.lock().await;
    match manager.list(params).await {
        Ok(result) => Ok(Json(ListResponse {
            success: true,
            data: Some(result),
            error: None,
        })),
        Err(e) => {
            error!("列出 TextBox 失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ListResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// 删除 TextBox
async fn delete_textbox(
    State(state): State<AnyboxState>,
    Path(id): Path<String>,
) -> Result<Json<TextBoxResponse>, (StatusCode, Json<TextBoxResponse>)> {
    info!("删除 TextBox: id={}", id);

    let mut manager = state.manager.lock().await;
    match manager.delete(&id).await {
        Ok(true) => Ok(Json(TextBoxResponse {
            success: true,
            data: None,
            error: None,
        })),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(TextBoxResponse {
                success: false,
                data: None,
                error: Some("TextBox 不存在".to_string()),
            }),
        )),
        Err(e) => {
            error!("删除 TextBox 失败: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TextBoxResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

/// 健康检查
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "anybox-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// 启动定时清理任务
pub fn start_cleanup_task(state: AnyboxState, interval_secs: u64) {
    info!("启动 Anybox 清理任务: 间隔={}秒", interval_secs);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            let mut manager = state.manager.lock().await;
            match manager.cleanup_expired().await {
                Ok(count) => {
                    if count > 0 {
                        info!("清理过期 TextBox: 删除 {} 个", count);
                    }
                }
                Err(e) => {
                    error!("清理过期 TextBox 失败: {}", e);
                }
            }
        }
    });
}

/// 创建 Anybox 路由
pub async fn create_routes(config: config::anybox::AnyboxConfig) -> anyhow::Result<Router> {
    // 异步初始化 state
    let state = AnyboxState::new(config.clone()).await?;

    // 启动清理任务
    start_cleanup_task(state.clone(), config.cleanup_interval_secs);

    Ok(Router::new()
        .route("/health", get(health_check))
        .route("/textbox", post(create_textbox))
        .route("/textbox", get(list_textboxes))
        .route("/textbox/:id", get(get_textbox))
        .route("/textbox/:id", axum::routing::delete(delete_textbox))
        .with_state(state))
}
