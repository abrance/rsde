# Kafka Docker 连接问题诊断和修复

## 🔍 问题诊断

你的 Kafka 配置：
```yaml
KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://${IP}:9092
```

**实际运行时的值：**
```
KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://:9092
```

**问题根源：** `${IP}` 环境变量未设置，导致 `ADVERTISED_LISTENERS` 为空地址。

### 为什么容器内可以，主机上不行？

- **容器内**：使用 `localhost:9092` 可以直接连接到容器内的 Kafka
- **主机上**：客户端连接后，Kafka 返回空的 advertised address，导致客户端无法建立后续连接

## ✅ 解决方案

### 方案 1：设置 IP 环境变量（推荐）

在启动 Kafka 前设置 IP 环境变量：

```bash
cd /opt/mystorage/github/tools/deploy/kafka

# 设置 IP 环境变量
export IP=$(hostname -I | awk '{print $1}')
echo "IP=$IP"

# 重启 Kafka
docker-compose down
docker-compose up -d

# 等待 Kafka 启动
sleep 20

# 验证配置
docker exec kafka-kafka-1 env | grep KAFKA_ADVERTISED_LISTENERS

# 测试连接
cd /opt/mystorage/github/rsde/py1015
python test_connection.py
```

### 方案 2：修改 docker-compose.yml 使用 localhost

如果只需要本机访问，修改配置为：

```yaml
version: '3.8'
services:
  zookeeper:
    image: wurstmeister/zookeeper
    ports:
      - "2181:2181"
    restart: always
  kafka:
    image: wurstmeister/kafka:2.13-2.8.1
    ports:
      - "9092:9092"
    environment:
      - KAFKA_ZOOKEEPER_CONNECT=zookeeper:2181
      - KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://localhost:9092
      - KAFKA_LISTENERS=PLAINTEXT://:9092
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    restart: always
```

### 方案 3：使用 .env 文件

在 `/opt/mystorage/github/tools/deploy/kafka/` 目录创建 `.env` 文件：

```bash
# 创建 .env 文件
cd /opt/mystorage/github/tools/deploy/kafka
echo "IP=$(hostname -I | awk '{print $1}')" > .env

# 查看内容
cat .env

# 重启
docker-compose down
docker-compose up -d
```

### 方案 4：使用固定 IP 地址

直接修改 docker-compose.yml：

```yaml
- KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://10.45.53.44:9092
```

**注意：** 如果主机 IP 变化，需要重新配置。

## 🧪 验证修复

修复后，运行以下命令验证：

```bash
# 1. 检查 Kafka 环境变量
docker exec kafka-kafka-1 env | grep KAFKA_ADVERTISED_LISTENERS

# 应该看到类似：
# KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://10.45.53.44:9092
# 或
# KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://localhost:9092

# 2. 测试连接
cd /opt/mystorage/github/rsde/py1015
python test_connection.py

# 3. 测试 Kafka 管理工具
python kafka_manager.py list

# 4. 创建测试主题
python kafka_manager.py create --topic test-topic
```

## 📝 快速修复命令

复制粘贴执行：

```bash
# 停止当前 Kafka
cd /opt/mystorage/github/tools/deploy/kafka
docker-compose down

# 创建 .env 文件
echo "IP=$(hostname -I | awk '{print $1}')" > .env
cat .env

# 重启 Kafka
docker-compose up -d

# 等待启动
echo "等待 Kafka 启动..."
sleep 25

# 验证配置
echo "验证 Kafka 配置："
docker exec kafka-kafka-1 env | grep KAFKA_ADVERTISED_LISTENERS

# 测试连接
cd /opt/mystorage/github/rsde/py1015
python test_connection.py
```

## 🎯 推荐方案

**建议使用方案 1（设置 IP 环境变量）或方案 2（使用 localhost）**

- 方案 1：适合需要其他机器访问的场景
- 方案 2：最简单，适合只在本机访问的场景

## 📚 技术细节

### ADVERTISED_LISTENERS 的作用

1. 客户端连接 `localhost:9092`
2. Kafka 返回元数据，包含 `ADVERTISED_LISTENERS`
3. 客户端使用这个地址进行后续通信
4. 如果地址为空或不可达，连接失败 → **NodeNotReadyError**

### 为什么需要正确配置

```
客户端 → Kafka (9092)
          ↓
     返回: "请连接到 :9092"  ← 错误！地址为空
          ↓
     NodeNotReadyError
```

正确配置后：
```
客户端 → Kafka (9092)
          ↓
     返回: "请连接到 localhost:9092"  ← 正确！
          ↓
     成功连接
```
