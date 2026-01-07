## 概述

rc 目的是开发一个远程测试一些中间件连接功能的工具。

特性:

- 用户友好
- 轻量级
- 支持设置超时
- 支持benchmark模式(类似ab)

## 架构

对接分为多个阶段

- dns解析
- ping
- 连接组件
- 读写数据

每个阶段会有独立的超时设置。


## 预计组件

Protocols:
- PureTCP
- HTTP
- SAMBA
- SSH
- FTP
- SFTP

Databases:
- PostgreSQL
- MySQL
- Redis
- MongoDB
- ES
- Doris
- InfluxDB
- VictoriaMetrics
- ClickHouse

Message Queues:
- RabbitMQ
- Kafka ✅

## 已实现功能

### Kafka

#### Ping 命令

测试 Kafka 集群的连接性。

**基本用法：**

```bash
# 不带认证的 ping
rc kafka ping -b localhost:9092

# 带 SASL PLAINTEXT 认证的 ping
rc kafka ping -b kafka.example.com:9092 \
  --sasl \
  --username myuser \
  --password mypassword

# 指定 topic 获取详细 metadata
rc kafka ping -b kafka.example.com:9092 \
  --sasl \
  --username myuser \
  --password mypassword \
  --topic my_topic

# 使用多个 broker 地址
rc kafka ping -b broker1:9092,broker2:9092,broker3:9092 \
  --sasl \
  --username myuser \
  --password mypassword
```

**参数说明：**

- `-b, --brokers <BROKERS>` - Kafka broker 地址（逗号分隔，必填）
- `--client-id <CLIENT_ID>` - 客户端 ID（默认：rc-kafka-client）
- `--timeout <TIMEOUT>` - 连接超时时间（秒，默认：10）
- `--sasl` - 启用 SASL 认证
- `--username <USERNAME>` - SASL 用户名（启用 SASL 时必填）
- `--password <PASSWORD>` - SASL 密码（启用 SASL 时必填）
- `--security-protocol <PROTOCOL>` - 安全协议（默认：SASL_PLAINTEXT）
  - `SASL_PLAINTEXT` - SASL 明文传输
  - `SASL_SSL` - SASL + SSL 加密
- `--mechanism <MECHANISM>` - SASL 认证机制（默认：PLAIN）
  - `PLAIN` - 明文用户名密码
  - `SCRAM-SHA-256` - SCRAM-SHA-256 认证
  - `SCRAM-SHA-512` - SCRAM-SHA-512 认证
- `-t, --topic <TOPIC>` - 查询指定 topic 的 metadata（可选）
- `--format <FORMAT>` - 输出格式（默认：text）
  - `text` - 人类可读的文本格式
  - `json` - JSON 格式，适合程序解析

**示例输出（text 格式）：**

```
🔌 Connecting to Kafka cluster...
   Brokers: kafka.example.com:9092
   Client ID: rc-kafka-client
   SASL: Enabled
   Username: myuser
   Security Protocol: SASL_PLAINTEXT
   Mechanism: PLAIN

⏳ Pinging Kafka cluster...
✅ Ping successful!

📊 Fetching metadata for topic 'test_topic'...

Cluster: sasl_plaintext://kafka-0.kafka-headless.default.svc.cluster.local:9092/0
Brokers: 2
Topics: 1
```

**示例输出（JSON 格式）：**

```bash
# JSON 输出示例
rc kafka ping -b kafka.example.com:9092 \
  --sasl --username myuser --password mypass \
  --topic test_topic \
  --format json
```

```json
{
  "success": true,
  "brokers": [
    "kafka.example.com:9092"
  ],
  "client_id": "rc-kafka-client",
  "sasl_enabled": true,
  "username": "myuser",
  "security_protocol": "SASL_PLAINTEXT",
  "mechanism": "PLAIN",
  "cluster_name": "sasl_plaintext://kafka-0.kafka-headless.default.svc.cluster.local:9092/0",
  "broker_count": 2,
  "topic_count": 1,
  "topic": "test_topic"
}
```

**失败场景的 JSON 输出：**

```json
{
  "success": false,
  "brokers": [
    "invalid-broker:9999"
  ],
  "client_id": "rc-kafka-client",
  "sasl_enabled": false,
  "error": "Ping failed: Meta data fetch error: BrokerTransportFailure (Local: Broker transport failure)"
}
```


