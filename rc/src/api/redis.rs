use axum::{
    Router,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use util::client::redis::{RedisClient, RedisClientConfig, RedisPingResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisPingRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

fn default_db() -> i64 {
    0
}

fn default_timeout() -> u64 {
    10
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisGetRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Key to get
    pub key: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisSetRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Key to set
    pub key: String,
    /// Value to set
    pub value: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// TTL in seconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisDelRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Key to delete
    pub key: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisInfoRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// Info section (server, clients, memory, stats, replication, cpu, keyspace, all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisKeysRequest {
    /// Redis server address (host:port or redis://host:port)
    pub host: String,
    /// Pattern to match (default: *)
    #[serde(default = "default_pattern")]
    pub pattern: String,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Username for ACL authentication (Redis 6.0+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Database index (default: 0)
    #[serde(default = "default_db")]
    pub db: i64,
    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

fn default_pattern() -> String {
    "*".to_string()
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rc-redis-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Redis ping API - test connectivity
async fn ping_redis(
    Json(req): Json<RedisPingRequest>,
) -> Result<Json<RedisPingResult>, (StatusCode, Json<RedisPingResult>)> {
    info!(
        "Redis ping request: host={}, db={}, tls={}",
        req.host, req.db, req.tls
    );

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls)
        .with_timeout(req.timeout);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut result = RedisPingResult {
        success: false,
        url: req.host.clone(),
        db: Some(req.db),
        version: None,
        dbsize: None,
        error: None,
    };

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            result.error = Some(e);
            error!("redis ping fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(result)));
        }
    };

    // Ping
    match client.ping().await {
        Ok(_) => {
            result.success = true;
        }
        Err(e) => {
            result.error = Some(e);
            error!("redis ping fail: {}", result.error.as_ref().unwrap());
            return Err((StatusCode::SERVICE_UNAVAILABLE, Json(result)));
        }
    }

    // Get version
    if let Ok(version) = client.version().await {
        result.version = Some(version);
    }

    // Get database size
    if let Ok(dbsize) = client.dbsize().await {
        result.dbsize = Some(dbsize);
    }

    info!("redis ping success: version={:?}", result.version);
    Ok(Json(result))
}

/// Redis get API
async fn get_redis(
    Json(req): Json<RedisGetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Redis get request: host={}, key={}", req.host, req.key);

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis get fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    match client.get(&req.key).await {
        Ok(Some(value)) => {
            let response = serde_json::json!({
                "key": req.key,
                "value": value,
                "exists": true
            });
            Ok(Json(response))
        }
        Ok(None) => {
            let response = serde_json::json!({
                "key": req.key,
                "value": null,
                "exists": false
            });
            Ok(Json(response))
        }
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis get fail: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// Redis set API
async fn set_redis(
    Json(req): Json<RedisSetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Redis set request: host={}, key={}", req.host, req.key);

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"success": false, "error": e});
            error!("redis set fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    let result = if let Some(ttl) = req.ttl {
        client.set_ex(&req.key, &req.value, ttl).await
    } else {
        client.set(&req.key, &req.value).await
    };

    match result {
        Ok(()) => {
            let response = serde_json::json!({
                "success": true,
                "key": req.key,
                "ttl": req.ttl
            });
            Ok(Json(response))
        }
        Err(e) => {
            let error_response = serde_json::json!({"success": false, "error": e});
            error!("redis set fail: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// Redis delete API
async fn del_redis(
    Json(req): Json<RedisDelRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Redis del request: host={}, key={}", req.host, req.key);

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis del fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    match client.del(&req.key).await {
        Ok(count) => {
            let response = serde_json::json!({
                "deleted": count,
                "key": req.key
            });
            Ok(Json(response))
        }
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis del fail: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// Redis info API
async fn info_redis(
    Json(req): Json<RedisInfoRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Redis info request: host={}", req.host);

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis info fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    match client.info(req.section.as_deref()).await {
        Ok(info) => {
            // Parse info into JSON format
            let info_map: std::collections::HashMap<String, String> = info
                .lines()
                .filter(|line| !line.starts_with('#') && line.contains(':'))
                .filter_map(|line| {
                    let mut parts = line.splitn(2, ':');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect();
            Ok(Json(serde_json::json!(info_map)))
        }
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis info fail: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// Redis keys API
async fn keys_redis(
    Json(req): Json<RedisKeysRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!(
        "Redis keys request: host={}, pattern={}",
        req.host, req.pattern
    );

    let mut config = RedisClientConfig::new(&req.host)
        .with_db(req.db)
        .with_tls(req.tls);

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    let mut client = match RedisClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis keys fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    match client.keys(&req.pattern).await {
        Ok(keys) => {
            let response = serde_json::json!({
                "pattern": req.pattern,
                "count": keys.len(),
                "keys": keys
            });
            Ok(Json(response))
        }
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("redis keys fail: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

pub fn create_routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ping", post(ping_redis))
        .route("/get", post(get_redis))
        .route("/set", post(set_redis))
        .route("/del", post(del_redis))
        .route("/info", post(info_redis))
        .route("/keys", post(keys_redis))
}
