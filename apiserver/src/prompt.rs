use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use prompt::{PaginatedResult, PaginationParams, PromptCategory, PromptTemplate, PromptTemplateManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct PromptState {
    manager: Arc<Mutex<PromptTemplateManager>>,
}

impl PromptState {
    pub async fn new(config: config::prompt::PromptConfig) -> anyhow::Result<Self> {
        let manager = PromptTemplateManager::new(config).await?;
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePromptRequest {
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PromptResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PromptTemplate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListPromptResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PaginatedResult<PromptTemplate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

async fn create_prompt(
    State(state): State<PromptState>,
    Json(req): Json<CreatePromptRequest>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<PromptResponse>)> {
    info!("Creating PromptTemplate: name={}", req.name);

    let mut template = PromptTemplate::new(req.name, req.content);

    if let Some(description) = req.description {
        template = template.with_description(description);
    }

    if let Some(category) = req.category.and_then(|c| PromptCategory::parse(&c)) {
        template = template.with_category(category);
    }

    if !req.variables.is_empty() {
        template = template.with_variables(req.variables);
    }

    if !req.tags.is_empty() {
        template = template.with_tags(req.tags);
    }

    if let Some(created_by) = req.created_by {
        template = template.with_created_by(created_by);
    }

    let manager = state.manager.lock().await;
    match manager.create(template).await {
        Ok(created) => Ok(Json(PromptResponse {
            success: true,
            data: Some(created),
            error: None,
        })),
        Err(e) => {
            error!("Failed to create PromptTemplate: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

async fn get_prompt(
    State(state): State<PromptState>,
    Path(id): Path<String>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<PromptResponse>)> {
    info!("Getting PromptTemplate: id={}", id);

    let manager = state.manager.lock().await;
    match manager.get(&id).await {
        Ok(Some(template)) => Ok(Json(PromptResponse {
            success: true,
            data: Some(template),
            error: None,
        })),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(PromptResponse {
                success: false,
                data: None,
                error: Some("PromptTemplate not found".to_string()),
            }),
        )),
        Err(e) => {
            error!("Failed to get PromptTemplate: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

async fn list_prompts(
    State(state): State<PromptState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<ListPromptResponse>, (StatusCode, Json<ListPromptResponse>)> {
    info!(
        "Listing PromptTemplates: page={}, page_size={}, name={:?}",
        params.pagination.page, params.pagination.page_size, params.name
    );

    let manager = state.manager.lock().await;
    
    let result = if let Some(name) = params.name {
        manager.search_by_name(&name, params.pagination).await
    } else {
        manager.list(params.pagination).await
    };

    match result {
        Ok(result) => Ok(Json(ListPromptResponse {
            success: true,
            data: Some(result),
            error: None,
        })),
        Err(e) => {
            error!("Failed to list PromptTemplates: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ListPromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

async fn update_prompt(
    State(state): State<PromptState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePromptRequest>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<PromptResponse>)> {
    info!("Updating PromptTemplate: id={}", id);

    let manager = state.manager.lock().await;
    
    let existing = match manager.get(&id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some("PromptTemplate not found".to_string()),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ));
        }
    };

    let category = req
        .category
        .and_then(|c| PromptCategory::parse(&c))
        .unwrap_or(existing.category);

    let updated = PromptTemplate {
        id: existing.id,
        name: req.name,
        description: req.description,
        category,
        content: req.content,
        variables: req.variables,
        tags: req.tags,
        version: existing.version + 1,
        is_active: req.is_active.unwrap_or(existing.is_active),
        created_at: existing.created_at,
        updated_at: chrono::Utc::now(),
        created_by: existing.created_by,
    };

    match manager.update(updated).await {
        Ok(result) => Ok(Json(PromptResponse {
            success: true,
            data: Some(result),
            error: None,
        })),
        Err(e) => {
            error!("Failed to update PromptTemplate: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

async fn delete_prompt(
    State(state): State<PromptState>,
    Path(id): Path<String>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<PromptResponse>)> {
    info!("Deleting PromptTemplate: id={}", id);

    let manager = state.manager.lock().await;
    match manager.delete(&id).await {
        Ok(true) => Ok(Json(PromptResponse {
            success: true,
            data: None,
            error: None,
        })),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(PromptResponse {
                success: false,
                data: None,
                error: Some("PromptTemplate not found".to_string()),
            }),
        )),
        Err(e) => {
            error!("Failed to delete PromptTemplate: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PromptResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "prompt-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub async fn create_routes(config: config::prompt::PromptConfig) -> anyhow::Result<Router> {
    let state = PromptState::new(config).await?;

    Ok(Router::new()
        .route("/health", get(health_check))
        .route("/template", post(create_prompt))
        .route("/template", get(list_prompts))
        .route("/template/:id", get(get_prompt))
        .route("/template/:id", axum::routing::put(update_prompt))
        .route("/template/:id", axum::routing::delete(delete_prompt))
        .with_state(state))
}
