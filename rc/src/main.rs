use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use util::client::kafka::{KafkaClientConfig, KafkaProducer, SaslConfig};

#[derive(Parser)]
#[command(name = "rc")]
#[command(about = "Remote Control CLI Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Kafka related operations
    Kafka(KafkaArgs),
}

#[derive(Args)]
struct KafkaArgs {
    #[command(subcommand)]
    command: KafkaCommands,
}

#[derive(Subcommand)]
enum KafkaCommands {
    /// Ping Kafka cluster to check connectivity
    Ping(PingArgs),
}

#[derive(Args)]
struct PingArgs {
    /// Kafka broker addresses (comma-separated)
    #[arg(short, long, value_delimiter = ',', required = true)]
    brokers: Vec<String>,

    /// Client ID
    #[arg(long, default_value = "rc-kafka-client")]
    client_id: String,

    /// Connection timeout in seconds
    #[arg(long, default_value = "10")]
    timeout: u64,

    /// Enable SASL authentication
    #[arg(long)]
    sasl: bool,

    /// SASL username (required if --sasl is set)
    #[arg(long, required_if_eq("sasl", "true"))]
    username: Option<String>,

    /// SASL password (required if --sasl is set)
    #[arg(long, required_if_eq("sasl", "true"))]
    password: Option<String>,

    /// SASL security protocol (SASL_PLAINTEXT or SASL_SSL)
    #[arg(long, default_value = "SASL_PLAINTEXT")]
    security_protocol: String,

    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
    #[arg(long, default_value = "PLAIN")]
    mechanism: String,

    /// Topic to check metadata (optional)
    #[arg(short, long)]
    topic: Option<String>,

    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    format: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PingResult {
    success: bool,
    brokers: Vec<String>,
    client_id: String,
    sasl_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mechanism: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cluster_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    broker_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Kafka(kafka_args) => handle_kafka_command(kafka_args).await?,
    }

    Ok(())
}

async fn handle_kafka_command(args: KafkaArgs) -> anyhow::Result<()> {
    match args.command {
        KafkaCommands::Ping(ping_args) => handle_ping(ping_args).await?,
    }
    Ok(())
}

async fn handle_ping(args: PingArgs) -> anyhow::Result<()> {
    let is_json = args.format.to_lowercase() == "json";
    let brokers = args.brokers.clone();
    let client_id = args.client_id.clone();
    let sasl_enabled = args.sasl;

    if !is_json {
        println!("🔌 Connecting to Kafka cluster...");
        println!("   Brokers: {}", args.brokers.join(", "));
        println!("   Client ID: {}", args.client_id);
    }

    // 创建配置
    let mut config =
        KafkaClientConfig::new(args.brokers, args.client_id).with_timeout(args.timeout);

    let mut result = PingResult {
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
        topic: args.topic.clone(),
        partition_count: None,
        error: None,
    };

    // 如果启用 SASL，添加认证配置
    if args.sasl {
        let username = args
            .username
            .ok_or_else(|| anyhow::anyhow!("Username is required when SASL is enabled"))?;
        let password = args
            .password
            .ok_or_else(|| anyhow::anyhow!("Password is required when SASL is enabled"))?;

        result.username = Some(username.clone());
        result.security_protocol = Some(args.security_protocol.clone());
        result.mechanism = Some(args.mechanism.clone());

        if !is_json {
            println!("   SASL: Enabled");
            println!("   Username: {}", username);
            println!("   Security Protocol: {}", args.security_protocol);
            println!("   Mechanism: {}", args.mechanism);
        }

        let sasl_config = SaslConfig {
            mechanism: args.mechanism,
            username,
            password,
            security_protocol: args.security_protocol,
        };
        config = config.with_sasl(sasl_config);
    }

    // 创建生产者
    let producer = match KafkaProducer::new(&config) {
        Ok(p) => p,
        Err(e) => {
            result.error = Some(format!("Failed to create Kafka producer: {}", e));
            if is_json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("❌ Error: {}", result.error.as_ref().unwrap());
            }
            return Err(anyhow::anyhow!("Failed to create producer"));
        }
    };

    // 执行 ping
    if !is_json {
        println!("\n⏳ Pinging Kafka cluster...");
    }

    match producer.ping(Duration::from_secs(args.timeout)) {
        Ok(_) => {
            result.success = true;
            if !is_json {
                println!("✅ Ping successful!\n");
            }
        }
        Err(e) => {
            result.error = Some(format!("Ping failed: {}", e));
            if is_json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("❌ Ping failed: {}", e);
            }
            return Err(anyhow::anyhow!("Ping failed"));
        }
    }

    // 如果指定了 topic，获取 topic metadata
    if let Some(topic) = &args.topic {
        if !is_json {
            println!("📊 Fetching metadata for topic '{}'...", topic);
        }
        match producer.get_topic_metadata(topic, Duration::from_secs(args.timeout)) {
            Ok(metadata) => {
                // 解析 metadata 字符串
                parse_metadata(&metadata, &mut result);
                if !is_json {
                    println!("\n{}", metadata);
                }
            }
            Err(e) => {
                if !is_json {
                    println!("⚠️  Failed to fetch topic metadata: {}", e);
                }
            }
        }
    } else {
        // 获取集群整体 metadata
        if !is_json {
            println!("📊 Fetching cluster metadata...");
        }
        match producer.get_topic_metadata("", Duration::from_secs(args.timeout)) {
            Ok(metadata) => {
                parse_metadata(&metadata, &mut result);
                if !is_json {
                    println!("\n{}", metadata);
                }
            }
            Err(_) => {
                if !is_json {
                    println!("   Use --topic <name> to get specific topic metadata\n");
                }
            }
        }
    }

    // 输出 JSON 结果
    if is_json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

fn parse_metadata(metadata: &str, result: &mut PingResult) {
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
    // 查找 partition 信息
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
