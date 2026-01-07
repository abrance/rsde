use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Message};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaClientConfig {
    /// Kafka 服务器地址列表
    pub brokers: Vec<String>,
    /// 客户端 ID
    pub client_id: String,
    /// 连接超时时间（秒）
    pub timeout: Option<u64>,
    /// 消费者组 ID（仅消费者需要）
    pub group_id: Option<String>,
    /// 会话超时时间（毫秒）
    pub session_timeout_ms: Option<u64>,
    /// 是否启用自动提交偏移量
    pub enable_auto_commit: Option<bool>,
    /// SASL 认证配置
    pub sasl_config: Option<SaslConfig>,
}

/// SASL 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaslConfig {
    /// SASL 机制（如 PLAIN, SCRAM-SHA-256, SCRAM-SHA-512）
    pub mechanism: String,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 安全协议（SASL_PLAINTEXT 或 SASL_SSL）
    pub security_protocol: String,
}

impl SaslConfig {
    /// 创建 SASL PLAINTEXT 配置
    pub fn plaintext(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mechanism: "PLAIN".to_string(),
            username: username.into(),
            password: password.into(),
            security_protocol: "SASL_PLAINTEXT".to_string(),
        }
    }

    /// 创建 SASL SSL 配置
    pub fn ssl(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mechanism: "PLAIN".to_string(),
            username: username.into(),
            password: password.into(),
            security_protocol: "SASL_SSL".to_string(),
        }
    }

    /// 设置 SASL 机制
    pub fn with_mechanism(mut self, mechanism: impl Into<String>) -> Self {
        self.mechanism = mechanism.into();
        self
    }
}

impl KafkaClientConfig {
    /// 创建一个新的 Kafka 客户端配置
    pub fn new(brokers: Vec<String>, client_id: impl Into<String>) -> Self {
        Self {
            brokers,
            client_id: client_id.into(),
            timeout: Some(30),
            group_id: None,
            session_timeout_ms: Some(6000),
            enable_auto_commit: Some(true),
            sasl_config: None,
        }
    }

    /// 设置连接超时时间（秒）
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置消费者组 ID
    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    /// 设置会话超时时间（毫秒）
    pub fn with_session_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.session_timeout_ms = Some(timeout_ms);
        self
    }

    /// 设置是否启用自动提交偏移量
    pub fn with_auto_commit(mut self, enable: bool) -> Self {
        self.enable_auto_commit = Some(enable);
        self
    }

    /// 设置 SASL 认证配置
    pub fn with_sasl(mut self, sasl_config: SaslConfig) -> Self {
        self.sasl_config = Some(sasl_config);
        self
    }

    /// 设置 SASL PLAINTEXT 认证（快捷方法）
    pub fn with_sasl_plaintext(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.sasl_config = Some(SaslConfig::plaintext(username, password));
        self
    }

    /// 获取 broker 地址字符串
    fn broker_string(&self) -> String {
        self.brokers.join(",")
    }

    /// 应用 SASL 配置到 ClientConfig
    fn apply_sasl_config(&self, client_config: &mut ClientConfig) {
        if let Some(sasl) = &self.sasl_config {
            client_config
                .set("security.protocol", &sasl.security_protocol)
                .set("sasl.mechanism", &sasl.mechanism)
                .set("sasl.username", &sasl.username)
                .set("sasl.password", &sasl.password);
        }
    }
}

/// Kafka 生产者客户端
pub struct KafkaProducer {
    producer: FutureProducer,
}

impl KafkaProducer {
    /// 创建一个新的 Kafka 生产者
    pub fn new(config: &KafkaClientConfig) -> Result<Self, String> {
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", config.broker_string())
            .set("client.id", &config.client_id)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "100000")
            .set("queue.buffering.max.kbytes", "1048576")
            .set("batch.num.messages", "10000");

        if let Some(timeout) = config.timeout {
            client_config.set("socket.timeout.ms", &(timeout * 1000).to_string());
        }

        // 应用 SASL 认证配置
        config.apply_sasl_config(&mut client_config);

        let producer = client_config
            .create()
            .map_err(|e| format!("Failed to create Kafka producer: {}", e))?;

        Ok(Self { producer })
    }

    /// 发送消息到指定的 topic
    pub async fn send(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), String> {
        let mut record = FutureRecord::to(topic).payload(payload);

        if let Some(k) = key {
            record = record.key(k);
        }

        self.producer
            .send(record, Timeout::After(Duration::from_secs(5)))
            .await
            .map_err(|(e, _)| format!("Failed to send message: {}", e))?;

        Ok(())
    }

    /// 发送 JSON 消息到指定的 topic
    pub async fn send_json<T: Serialize>(
        &self,
        topic: &str,
        key: Option<&str>,
        value: &T,
    ) -> Result<(), String> {
        let payload =
            serde_json::to_vec(value).map_err(|e| format!("Failed to serialize message: {}", e))?;
        self.send(topic, key, &payload).await
    }

    /// 刷新缓冲区，确保所有消息已发送
    pub fn flush(&self, timeout: Duration) -> Result<(), String> {
        self.producer
            .flush(Timeout::After(timeout))
            .map_err(|e| format!("Failed to flush producer: {}", e))?;
        Ok(())
    }
}

/// Kafka 消费者客户端
pub struct KafkaConsumer {
    consumer: StreamConsumer,
}

impl KafkaConsumer {
    /// 创建一个新的 Kafka 消费者
    pub fn new(config: &KafkaClientConfig) -> Result<Self, String> {
        let group_id = config
            .group_id
            .as_ref()
            .ok_or_else(|| "Group ID is required for consumer".to_string())?;

        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", config.broker_string())
            .set("client.id", &config.client_id)
            .set("group.id", group_id)
            .set(
                "enable.auto.commit",
                &config.enable_auto_commit.unwrap_or(true).to_string(),
            )
            .set(
                "session.timeout.ms",
                &config.session_timeout_ms.unwrap_or(6000).to_string(),
            )
            .set("enable.partition.eof", "false")
            .set("auto.offset.reset", "earliest");

        if let Some(timeout) = config.timeout {
            client_config.set("socket.timeout.ms", &(timeout * 1000).to_string());
        }

        // 应用 SASL 认证配置
        config.apply_sasl_config(&mut client_config);

        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| format!("Failed to create Kafka consumer: {}", e))?;

        Ok(Self { consumer })
    }

    /// 订阅指定的 topics
    pub fn subscribe(&self, topics: &[&str]) -> Result<(), String> {
        self.consumer
            .subscribe(topics)
            .map_err(|e| format!("Failed to subscribe to topics: {}", e))?;
        Ok(())
    }

    /// 接收下一条消息（阻塞式）
    pub async fn recv(&self) -> Result<BorrowedMessage<'_>, String> {
        self.consumer
            .recv()
            .await
            .map_err(|e| format!("Failed to receive message: {}", e))
    }

    /// 提交当前偏移量
    pub fn commit(&self) -> Result<(), String> {
        self.consumer
            .commit_consumer_state(rdkafka::consumer::CommitMode::Sync)
            .map_err(|e| format!("Failed to commit offset: {}", e))?;
        Ok(())
    }

    /// 获取内部的 StreamConsumer 引用（用于高级用法）
    pub fn inner(&self) -> &StreamConsumer {
        &self.consumer
    }
}

/// 辅助函数：从消息中提取 payload
pub fn extract_payload<'a>(msg: &'a BorrowedMessage<'a>) -> Option<&'a [u8]> {
    msg.payload()
}

/// 辅助函数：从消息中提取 payload 并反序列化为 JSON
pub fn extract_json<'a, T: Deserialize<'a>>(msg: &'a BorrowedMessage<'a>) -> Result<T, String> {
    let payload = extract_payload(msg).ok_or_else(|| "Message has no payload".to_string())?;
    serde_json::from_slice(payload).map_err(|e| format!("Failed to deserialize message: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_KAFKA_BROKERS: &str = "test-kafka.bkbase-test.svc.cluster.local:9092";
    const USERNAME: &str = "user1";
    const PASSWORD: &str = "testpass4user1";
    #[test]
    fn test_config_creation() {
        let config = KafkaClientConfig::new(vec!["localhost:9092".to_string()], "test-client")
            .with_timeout(60)
            .with_group_id("test-group");

        assert_eq!(config.broker_string(), "localhost:9092");
        assert_eq!(config.client_id, "test-client");
        assert_eq!(config.timeout, Some(60));
        assert_eq!(config.group_id, Some("test-group".to_string()));
    }

    #[test]
    fn test_config_with_sasl() {
        let config = KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-client")
            .with_sasl_plaintext(USERNAME, PASSWORD)
            .with_group_id("test-group");

        assert!(config.sasl_config.is_some());
        let sasl = config.sasl_config.unwrap();
        assert_eq!(sasl.mechanism, "PLAIN");
        assert_eq!(sasl.username, USERNAME);
        assert_eq!(sasl.password, PASSWORD);
        assert_eq!(sasl.security_protocol, "SASL_PLAINTEXT");
    }

    #[test]
    fn test_sasl_config_builder() {
        let sasl = SaslConfig::plaintext("user", "pass").with_mechanism("SCRAM-SHA-256");

        assert_eq!(sasl.mechanism, "SCRAM-SHA-256");
        assert_eq!(sasl.username, "user");
        assert_eq!(sasl.password, "pass");
    }
}
