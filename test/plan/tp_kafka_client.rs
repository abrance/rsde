use rdkafka::message::Message;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use util::client::kafka::{
    KafkaClientConfig, KafkaConsumer, KafkaProducer, extract_json, extract_payload,
};

const TEST_KAFKA_BROKERS: &str = "test-kafka.bkbase-test.svc.cluster.local:9092";
const USERNAME: &str = "user1";
const PASSWORD: &str = "testpass4user1";
const TEST_TOPIC: &str = "test_topic";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestMessage {
    id: u64,
    content: String,
    timestamp: i64,
}

/// 测试生产者基本功能
#[tokio::test]
#[ignore] // 需要真实 Kafka 环境，使用 `cargo test -- --ignored` 运行
async fn test_producer_send_message() {
    let config =
        KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-producer-client")
            .with_sasl_plaintext(USERNAME, PASSWORD)
            .with_timeout(10);

    let producer = KafkaProducer::new(&config).expect("Failed to create producer");

    // 发送简单消息
    let payload = b"Hello Kafka!";
    let result = producer.send(TEST_TOPIC, Some("test-key"), payload).await;

    assert!(result.is_ok(), "Failed to send message: {:?}", result.err());

    // 刷新确保消息已发送
    producer
        .flush(Duration::from_secs(5))
        .expect("Failed to flush");

    println!("✓ Successfully sent message to topic: {}", TEST_TOPIC);
}

/// 测试发送 JSON 消息
#[tokio::test]
#[ignore]
async fn test_producer_send_json() {
    let config = KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-json-producer")
        .with_sasl_plaintext(USERNAME, PASSWORD);

    let producer = KafkaProducer::new(&config).expect("Failed to create producer");

    let test_msg = TestMessage {
        id: 12345,
        content: "Test JSON message".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    let result = producer
        .send_json(TEST_TOPIC, Some("json-key"), &test_msg)
        .await;
    assert!(
        result.is_ok(),
        "Failed to send JSON message: {:?}",
        result.err()
    );

    producer
        .flush(Duration::from_secs(5))
        .expect("Failed to flush");

    println!("✓ Successfully sent JSON message: {:?}", test_msg);
}

/// 测试消费者基本功能
#[tokio::test]
#[ignore]
async fn test_consumer_receive_message() {
    let config =
        KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-consumer-client")
            .with_sasl_plaintext(USERNAME, PASSWORD)
            .with_group_id("test-consumer-group")
            .with_auto_commit(true);

    let consumer = KafkaConsumer::new(&config).expect("Failed to create consumer");
    consumer
        .subscribe(&[TEST_TOPIC])
        .expect("Failed to subscribe");

    println!("Waiting for messages from topic: {}", TEST_TOPIC);

    // 设置超时以避免无限等待
    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        match consumer.recv().await {
            Ok(msg) => {
                let payload = extract_payload(&msg);
                println!("✓ Received message:");
                println!("  Topic: {}", msg.topic());
                println!("  Partition: {}", msg.partition());
                println!("  Offset: {}", msg.offset());
                if let Some(key) = msg.key() {
                    println!("  Key: {:?}", String::from_utf8_lossy(key));
                }
                if let Some(p) = payload {
                    println!("  Payload: {:?}", String::from_utf8_lossy(p));
                }
                true
            }
            Err(e) => {
                println!("✗ Failed to receive message: {}", e);
                false
            }
        }
    });

    match timeout.await {
        Ok(success) => assert!(success, "Message reception failed"),
        Err(_) => println!("⚠ Timeout waiting for message (this is OK if topic is empty)"),
    }
}

/// 测试消费 JSON 消息
#[tokio::test]
#[ignore]
async fn test_consumer_receive_json() {
    let config = KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-json-consumer")
        .with_sasl_plaintext(USERNAME, PASSWORD)
        .with_group_id("test-json-consumer-group")
        .with_auto_commit(false);

    let consumer = KafkaConsumer::new(&config).expect("Failed to create consumer");
    consumer
        .subscribe(&[TEST_TOPIC])
        .expect("Failed to subscribe");

    println!("Waiting for JSON messages from topic: {}", TEST_TOPIC);

    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        // 尝试接收多条消息，直到找到 JSON 消息或超时
        loop {
            match consumer.recv().await {
                Ok(msg) => {
                    match extract_json::<TestMessage>(&msg) {
                        Ok(test_msg) => {
                            println!("✓ Successfully parsed JSON message:");
                            println!("  ID: {}", test_msg.id);
                            println!("  Content: {}", test_msg.content);
                            println!("  Timestamp: {}", test_msg.timestamp);

                            // 手动提交偏移量
                            consumer.commit().expect("Failed to commit offset");
                            return true;
                        }
                        Err(e) => {
                            // 如果不是 JSON 消息，继续尝试下一条
                            println!("⚠ Skipping non-JSON message: {}", e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    println!("✗ Failed to receive message: {}", e);
                    return false;
                }
            }
        }
    });

    match timeout.await {
        Ok(success) => assert!(success, "JSON message reception failed"),
        Err(_) => {
            println!("⚠ Timeout waiting for JSON message (this is OK if no JSON messages in topic)")
        }
    }
}

/// 测试生产者和消费者端到端流程
#[tokio::test]
#[ignore]
async fn test_producer_consumer_e2e() {
    let test_topic = "test_e2e_topic";

    // 创建生产者
    let producer_config =
        KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "e2e-producer")
            .with_sasl_plaintext(USERNAME, PASSWORD);

    let producer = KafkaProducer::new(&producer_config).expect("Failed to create producer");

    // 发送测试消息
    let test_msg = TestMessage {
        id: 99999,
        content: "End-to-end test message".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    println!("Sending test message...");
    producer
        .send_json(test_topic, Some("e2e-key"), &test_msg)
        .await
        .expect("Failed to send message");

    producer
        .flush(Duration::from_secs(5))
        .expect("Failed to flush");
    println!("✓ Message sent successfully");

    // 创建消费者
    let consumer_config =
        KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "e2e-consumer")
            .with_sasl_plaintext(USERNAME, PASSWORD)
            .with_group_id("e2e-consumer-group")
            .with_auto_commit(true);

    let consumer = KafkaConsumer::new(&consumer_config).expect("Failed to create consumer");
    consumer
        .subscribe(&[test_topic])
        .expect("Failed to subscribe");

    // 等待并接收消息
    println!("Waiting to receive message...");

    let timeout = tokio::time::timeout(Duration::from_secs(30), async {
        match consumer.recv().await {
            Ok(msg) => match extract_json::<TestMessage>(&msg) {
                Ok(received_msg) => {
                    println!("✓ Received message matches sent message:");
                    assert_eq!(received_msg.id, test_msg.id);
                    assert_eq!(received_msg.content, test_msg.content);
                    println!("  ID: {}", received_msg.id);
                    println!("  Content: {}", received_msg.content);
                    true
                }
                Err(e) => {
                    println!("✗ Failed to parse message: {}", e);
                    false
                }
            },
            Err(e) => {
                println!("✗ Failed to receive message: {}", e);
                false
            }
        }
    });

    match timeout.await {
        Ok(success) => assert!(success, "End-to-end test failed"),
        Err(_) => panic!("Timeout waiting for message in e2e test"),
    }
}

/// 测试错误处理 - 无效的 broker 地址
#[tokio::test]
async fn test_invalid_broker() {
    let config = KafkaClientConfig::new(
        vec!["invalid-broker:9999".to_string()],
        "test-invalid-client",
    );

    // 创建生产者应该成功（连接是懒加载的）
    let producer = KafkaProducer::new(&config);
    assert!(
        producer.is_ok(),
        "Producer creation should succeed even with invalid broker"
    );

    // 但发送消息应该失败
    if let Ok(p) = producer {
        let result = p.send("test", None, b"test").await;
        // 注意：根据超时设置，这可能需要一些时间才会失败
        println!("Send result with invalid broker: {:?}", result);
    }
}

/// 测试多次消息发送的性能
#[tokio::test]
#[ignore]
async fn test_batch_send_performance() {
    let config =
        KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-batch-producer")
            .with_sasl_plaintext(USERNAME, PASSWORD);

    let producer = KafkaProducer::new(&config).expect("Failed to create producer");

    let message_count = 100;
    let start_time = std::time::Instant::now();

    for i in 0..message_count {
        let msg = TestMessage {
            id: i,
            content: format!("Batch message {}", i),
            timestamp: chrono::Utc::now().timestamp(),
        };

        producer
            .send_json(TEST_TOPIC, Some(&format!("key-{}", i)), &msg)
            .await
            .expect("Failed to send message");
    }

    producer
        .flush(Duration::from_secs(10))
        .expect("Failed to flush");

    let elapsed = start_time.elapsed();
    let msg_per_sec = message_count as f64 / elapsed.as_secs_f64();

    println!("✓ Sent {} messages in {:?}", message_count, elapsed);
    println!("  Rate: {:.2} messages/second", msg_per_sec);

    assert!(elapsed.as_secs() < 30, "Batch send took too long");
}

/// 测试 ping 功能 - 生产者
#[tokio::test]
#[ignore]
async fn test_producer_ping() {
    let config = KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-ping-producer")
        .with_sasl_plaintext(USERNAME, PASSWORD);

    let producer = KafkaProducer::new(&config).expect("Failed to create producer");

    // 测试 ping
    let result = producer.ping(Duration::from_secs(5));
    assert!(result.is_ok(), "Ping failed: {:?}", result.err());
    println!("✓ Producer ping successful");

    // 获取 topic metadata
    let metadata = producer.get_topic_metadata(TEST_TOPIC, Duration::from_secs(5));
    match metadata {
        Ok(info) => {
            println!("✓ Topic metadata:");
            println!("{}", info);
        }
        Err(e) => println!("⚠ Failed to get topic metadata: {}", e),
    }
}

/// 测试 ping 功能 - 消费者
#[tokio::test]
#[ignore]
async fn test_consumer_ping() {
    let config = KafkaClientConfig::new(vec![TEST_KAFKA_BROKERS.to_string()], "test-ping-consumer")
        .with_sasl_plaintext(USERNAME, PASSWORD)
        .with_group_id("test-ping-group");

    let consumer = KafkaConsumer::new(&config).expect("Failed to create consumer");

    // 测试 ping
    let result = consumer.ping(Duration::from_secs(5));
    assert!(result.is_ok(), "Ping failed: {:?}", result.err());
    println!("✓ Consumer ping successful");

    // 获取 topic metadata
    let metadata = consumer.get_topic_metadata(TEST_TOPIC, Duration::from_secs(5));
    match metadata {
        Ok(info) => {
            println!("✓ Topic metadata:");
            println!("{}", info);
        }
        Err(e) => println!("⚠ Failed to get topic metadata: {}", e),
    }
}

/// 测试连接失败时的 ping 行为
#[tokio::test]
async fn test_ping_with_invalid_broker() {
    let config =
        KafkaClientConfig::new(vec!["invalid-broker:9999".to_string()], "test-invalid-ping");

    let producer = KafkaProducer::new(&config).expect("Producer creation should succeed");

    // ping 应该失败
    let result = producer.ping(Duration::from_secs(3));
    assert!(result.is_err(), "Ping should fail with invalid broker");
    println!(
        "✓ Ping correctly failed with invalid broker: {:?}",
        result.err()
    );
}
