use axum::{
    Router,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use util::client::mysql::{MySqlClient, MySqlClientConfig, MySqlPingResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct MySqlPingRequest {
    /// MySQL server address (host:port or host)
    pub host: String,
    /// Username for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Database name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Enable SSL/TLS
    #[serde(default)]
    pub ssl: bool,
}

fn default_timeout() -> u64 {
    10
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MySqlQueryRequest {
    /// MySQL server address (host:port or host)
    pub host: String,
    /// SQL query to execute
    pub query: String,
    /// Query type (ddl or dml)
    #[serde(default = "default_query_type")]
    pub query_type: String,
    /// Username for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Database name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Enable SSL/TLS
    #[serde(default)]
    pub ssl: bool,
}

fn default_query_type() -> String {
    "dml".to_string()
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rc-mysql-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// MySQL ping API - test connectivity
async fn ping_mysql(
    Json(req): Json<MySqlPingRequest>,
) -> Result<Json<MySqlPingResult>, (StatusCode, Json<MySqlPingResult>)> {
    info!(
        "MySQL ping request: host={}, database={:?}, ssl={}",
        req.host, req.database, req.ssl
    );

    let mut config = MySqlClientConfig::new(&req.host).with_timeout(req.timeout);

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(database) = &req.database {
        config = config.with_database(database);
    }

    if req.ssl {
        config = config.with_ssl(true);
    }

    let mut result = MySqlPingResult {
        success: false,
        host: req.host.clone(),
        port: 3306,
        database: req.database.clone(),
        version: None,
        error: None,
    };

    // Parse port from host if present
    if let Some(pos) = req.host.find(':') {
        if let Ok(port) = req.host[pos + 1..].parse() {
            result.port = port;
        }
    }

    let mut client = match MySqlClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            result.error = Some(e);
            error!("mysql ping fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(result)));
        }
    };

    // Ping
    match client.ping().await {
        Ok(()) => {
            result.success = true;
        }
        Err(e) => {
            result.error = Some(e);
            error!("mysql ping fail: {}", result.error.as_ref().unwrap());
            return Err((StatusCode::SERVICE_UNAVAILABLE, Json(result)));
        }
    }

    // Get version
    if let Ok(version) = client.version().await {
        result.version = Some(version);
    }

    info!("mysql ping success: version={:?}", result.version);
    Ok(Json(result))
}

/// MySQL query API - execute SQL queries
async fn query_mysql(
    Json(req): Json<MySqlQueryRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!(
        "MySQL query request: host={}, query_type={}, query_length={}",
        req.host,
        req.query_type,
        req.query.len()
    );

    let mut config = MySqlClientConfig::new(&req.host).with_timeout(req.timeout);

    if let Some(username) = &req.username {
        config = config.with_username(username);
    }

    if let Some(password) = &req.password {
        config = config.with_password(password);
    }

    if let Some(database) = &req.database {
        config = config.with_database(database);
    }

    if req.ssl {
        config = config.with_ssl(true);
    }

    let mut client = match MySqlClient::new(&config).await {
        Ok(c) => c,
        Err(e) => {
            let error_response = serde_json::json!({"error": e});
            error!("mysql query fail: connection error");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    let query_type = req.query_type.to_lowercase();
    match query_type.as_str() {
        "ddl" => match client.execute_ddl(&req.query).await {
            Ok(()) => {
                let response = serde_json::json!({
                    "success": true,
                    "message": "DDL executed successfully"
                });
                Ok(Json(response))
            }
            Err(e) => {
                let error_response = serde_json::json!({"error": e});
                error!("mysql ddl query fail: {}", e);
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        },
        "dml" | _ => match client.execute_dml(&req.query).await {
            Ok(rows_affected) => {
                let response = serde_json::json!({
                    "success": true,
                    "rows_affected": rows_affected
                });
                Ok(Json(response))
            }
            Err(e) => {
                let error_response = serde_json::json!({"error": e});
                error!("mysql dml query fail: {}", e);
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
            }
        },
    }
}

pub fn create_routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ping", post(ping_mysql))
        .route("/query", post(query_mysql))
}
