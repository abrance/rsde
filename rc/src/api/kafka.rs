use axum::{
    Router,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info};
use util::client::kafka::{KafkaClientConfig, KafkaProducer, SaslConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct PingRequest {
    pub brokers: Vec<String>,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub sasl: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default = "default_security_protocol")]
    pub security_protocol: String,
    #[serde(default = "default_mechanism")]
    pub mechanism: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

fn default_client_id() -> String {
    "rc-api-kafka-client".to_string()
}

fn default_timeout() -> u64 {
    10
}

fn default_security_protocol() -> String {
    "SASL_PLAINTEXT".to_string()
}

fn default_mechanism() -> String {
    "PLAIN".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingResponse {
    pub success: bool,
    pub brokers: Vec<String>,
    pub client_id: String,
    pub sasl_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanism: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rc-kafka-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Kafka ping API
async fn ping_kafka(
    Json(req): Json<PingRequest>,
) -> Result<Json<PingResponse>, (StatusCode, Json<PingResponse>)> {
    info!(
        "Kafka ping request: brokers={:?}, sasl={}",
        req.brokers, req.sasl
    );

    let brokers = req.brokers.clone();
    let client_id = req.client_id.clone();
    let sasl_enabled = req.sasl;

    let mut config = KafkaClientConfig::new(req.brokers.clone(), req.client_id.clone())
        .with_timeout(req.timeout);

    let mut result = PingResponse {
        success: false,
        brokers: brokers.clone(),
        client_id: client_id.clone(),
        sasl_enabled,
        username: None,
        security_protocol: None,
        mechanism: None,
        cluster_name: None,
        broker_count: None,
        topic_count: None,
        topic: req.topic.clone(),
        partition_count: None,
        error: None,
    };

    // SASL 配置
    if req.sasl {
        if req.username.is_none() || req.password.is_none() {
            result.error = Some("username and password required when sasl enabled".to_string());
            error!("kafka ping fail: sasl enabled but missing credentials");
            return Err((StatusCode::BAD_REQUEST, Json(result)));
        }

        let username = req.username.clone().unwrap();
        let password = req.password.unwrap();

        result.username = Some(username.clone());
        result.security_protocol = Some(req.security_protocol.clone());
        result.mechanism = Some(req.mechanism.clone());

        let sasl_config = SaslConfig {
            mechanism: req.mechanism.clone(),
            username,
            password,
            security_protocol: req.security_protocol.clone(),
        };
        config = config.with_sasl(sasl_config);
    }

    // 创建 producer
    let producer = match KafkaProducer::new(&config) {
        Ok(p) => p,
        Err(e) => {
            result.error = Some(format!("create producer fail: {}", e));
            error!("kafka ping fail: create producer error, {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(result)));
        }
    };

    // Ping
    match producer.ping(Duration::from_secs(req.timeout)) {
        Ok(_) => {
            result.success = true;
        }
        Err(e) => {
            result.error = Some(format!("ping fail: {}", e));
            error!("kafka ping fail: {}", e);
            return Err((StatusCode::SERVICE_UNAVAILABLE, Json(result)));
        }
    }

    // 获取 metadata
    if let Some(topic) = &req.topic {
        match producer.get_topic_metadata(topic, Duration::from_secs(req.timeout)) {
            Ok(metadata) => {
                parse_metadata(&metadata, &mut result);
            }
            Err(e) => {
                error!("fetch topic metadata fail: {}", e);
            }
        }
    } else {
        match producer.get_topic_metadata("", Duration::from_secs(req.timeout)) {
            Ok(metadata) => {
                parse_metadata(&metadata, &mut result);
            }
            Err(e) => {
                error!("fetch cluster metadata fail: {}", e);
            }
        }
    }

    info!("kafka ping success: cluster={:?}", result.cluster_name);
    Ok(Json(result))
}

fn parse_metadata(metadata: &str, result: &mut PingResponse) {
    if let Some(cluster_line) = metadata.lines().next() {
        if let Some(cluster) = cluster_line.strip_prefix("Cluster: ") {
            result.cluster_name = Some(cluster.to_string());
        }
    }
    if let Some(brokers_line) = metadata.lines().nth(1) {
        if let Some(count_str) = brokers_line.strip_prefix("Brokers: ") {
            result.broker_count = count_str.parse().ok();
        }
    }
    if let Some(topics_line) = metadata.lines().nth(2) {
        if let Some(count_str) = topics_line.strip_prefix("Topics: ") {
            result.topic_count = count_str.parse().ok();
        }
    }
    for line in metadata.lines() {
        if line.contains("partitions") {
            if let Some(parts) = line.split(':').nth(1) {
                if let Some(count_str) = parts.trim().split_whitespace().next() {
                    result.partition_count = count_str.parse().ok();
                }
            }
        }
    }
}

pub fn create_routes() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ping", post(ping_kafka))
}
