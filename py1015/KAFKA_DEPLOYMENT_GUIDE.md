# Kafka 部署配置对比

## 目录结构

```
/opt/mystorage/github/tools/deploy/kafka/
├── no_auth/              # 无认证配置
│   ├── docker-compose.yml
│   └── .env
│
└── plain/                # SASL/PLAIN 认证配置
    ├── docker-compose.yml
    ├── kafka_server_jaas.conf
    ├── .env
    ├── README.md
    └── start_kafka_sasl.sh
```

## 配置对比

### 1. 无认证 (no_auth)

**特点:**
- ✅ 简单易用，无需认证
- ✅ 适合开发和测试环境
- ❌ 不安全，不适合生产环境

**连接方式:**
```python
from kafka.admin import KafkaAdminClient

admin_client = KafkaAdminClient(
    bootstrap_servers='localhost:9092'
)
```

**命令行:**
```bash
python kafka_manager.py list --server localhost:9092
```

### 2. SASL/PLAIN 认证 (plain)

**特点:**
- ✅ 基础的用户名密码认证
- ✅ 支持多用户管理
- ⚠️  密码明文传输（建议使用 SASL_SSL）
- ✅ 适合内网测试环境

**默认用户:**
| 用户名 | 密码 | 说明 |
|--------|------|------|
| admin | admin-secret | 管理员 |
| producer | producer-secret | 生产者 |
| consumer | consumer-secret | 消费者 |

**连接方式:**
```python
from kafka.admin import KafkaAdminClient

admin_client = KafkaAdminClient(
    bootstrap_servers='localhost:9092',
    security_protocol='SASL_PLAINTEXT',
    sasl_mechanism='PLAIN',
    sasl_plain_username='admin',
    sasl_plain_password='admin-secret'
)
```

**命令行:**
```bash
python kafka_manager.py list \
  --security-protocol SASL_PLAINTEXT \
  --sasl-mechanism PLAIN \
  --sasl-username admin \
  --sasl-password admin-secret
```

## 快速启动

### 启动无认证 Kafka

```bash
cd /opt/mystorage/github/tools/deploy/kafka/no_auth
docker-compose down
docker-compose up -d
sleep 30

# 测试
cd /opt/mystorage/github/rsde/py1015
python test_connection.py
python kafka_manager.py list
```

### 启动带认证 Kafka

```bash
cd /opt/mystorage/github/tools/deploy/kafka/plain
./start_kafka_sasl.sh

# 或手动启动
docker-compose down
docker-compose up -d
sleep 35

# 测试
cd /opt/mystorage/github/rsde/py1015
python test_sasl_connection.py

# 使用
python kafka_manager.py list \
  --security-protocol SASL_PLAINTEXT \
  --sasl-mechanism PLAIN \
  --sasl-username admin \
  --sasl-password admin-secret
```

## 切换配置

### 从无认证切换到认证

```bash
# 停止无认证 Kafka
cd /opt/mystorage/github/tools/deploy/kafka/no_auth
docker-compose down

# 启动认证 Kafka
cd /opt/mystorage/github/tools/deploy/kafka/plain
docker-compose up -d
```

### 从认证切换到无认证

```bash
# 停止认证 Kafka
cd /opt/mystorage/github/tools/deploy/kafka/plain
docker-compose down

# 启动无认证 Kafka
cd /opt/mystorage/github/tools/deploy/kafka/no_auth
docker-compose up -d
```

## 环境变量配置

两个配置都需要 `.env` 文件来设置 IP 地址：

```bash
# 本机访问
IP=localhost

# 远程访问（替换为实际 IP）
IP=10.45.53.44
```

## Python 客户端工具

在 `/opt/mystorage/github/rsde/py1015` 目录下：

| 工具 | 说明 |
|------|------|
| `kafka_manager.py` | Kafka 主题管理工具（支持认证） |
| `test_connection.py` | 无认证连接测试 |
| `test_sasl_connection.py` | SASL 认证连接测试 |
| `requirements.txt` | Python 依赖 |

## 添加新用户 (SASL/PLAIN)

编辑 `/opt/mystorage/github/tools/deploy/kafka/plain/kafka_server_jaas.conf`:

```java
KafkaServer {
    org.apache.kafka.common.security.plain.PlainLoginModule required
    username="admin"
    password="admin-secret"
    user_admin="admin-secret"
    user_producer="producer-secret"
    user_consumer="consumer-secret"
    user_newuser="newpassword";  // 添加新用户
};
```

然后重启 Kafka:

```bash
cd /opt/mystorage/github/tools/deploy/kafka/plain
docker-compose restart kafka
sleep 20
```

## 故障排查

### 无法连接

1. 检查容器状态: `docker-compose ps`
2. 查看日志: `docker-compose logs kafka`
3. 验证端口: `netstat -tuln | grep 9092`
4. 检查 IP 配置: `cat .env`

### 认证失败

1. 检查用户名密码是否正确
2. 验证 JAAS 配置: `docker exec plain-kafka-1 cat /opt/kafka/config/kafka_server_jaas.conf`
3. 查看认证日志: `docker-compose logs kafka | grep -i authentication`
4. 重启 Kafka: `docker-compose restart kafka`

### NodeNotReadyError

1. 等待 30 秒后重试
2. 检查 advertised.listeners 配置
3. 验证 IP 地址是否正确

## 安全建议

### 开发/测试环境
- ✅ 使用 `no_auth` 配置（简单快速）
- ✅ 使用 `localhost` 作为 IP

### 内网测试环境
- ✅ 使用 `plain` 配置（SASL/PLAIN）
- ✅ 修改默认密码
- ✅ 使用实际 IP 地址

### 生产环境
- ❌ 不要使用这些配置
- ✅ 使用 SASL_SSL（加密传输）
- ✅ 使用 SCRAM-SHA-256/512（更安全的认证）
- ✅ 配置 ACL（访问控制列表）
- ✅ 启用 SSL/TLS 加密

## 端口说明

| 端口 | 服务 | 说明 |
|------|------|------|
| 2181 | Zookeeper | Kafka 依赖的协调服务 |
| 9092 | Kafka | Kafka broker 监听端口 |

## 相关文档

- `/opt/mystorage/github/tools/deploy/kafka/plain/README.md` - SASL/PLAIN 详细文档
- `/opt/mystorage/github/rsde/py1015/README.md` - Python 客户端使用文档
- `/opt/mystorage/github/rsde/py1015/fix_kafka_connection.md` - 连接问题修复指南
