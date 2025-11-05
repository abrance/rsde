# Kafka Topic Manager

这是一个用于管理 Kafka 主题的 Python 客户端工具。

## 功能特性

- ✅ 创建 Kafka 主题
- ✅ 列出所有主题
- ✅ 删除主题
- ✅ 查看主题元数据

## 系统要求

- Python 3.11+
- Kafka 服务器 (运行中)

## 安装依赖

```bash
pip install -r requirements.txt
```

主要依赖：
- `kafka-python==2.0.2` - Kafka Python 客户端库

## 使用方法

### 1. 列出所有主题

```bash
python kafka_manager.py list
```

### 2. 创建新主题

```bash
# 创建默认配置的主题（1个分区，1个副本）
python kafka_manager.py create --topic my-topic

# 创建自定义配置的主题
python kafka_manager.py create --topic my-topic --partitions 3 --replication-factor 2
```

### 3. 删除主题

```bash
python kafka_manager.py delete --topic my-topic
```

### 4. 查看主题元数据

```bash
python kafka_manager.py metadata --topic my-topic
```

### 5. 使用自定义 Kafka 服务器地址

```bash
python kafka_manager.py list --server kafka-broker:9092
python kafka_manager.py create --topic test --server 192.168.1.100:9092
```

## 命令行参数说明

| 参数 | 说明 | 默认值 | 是否必需 |
|------|------|--------|----------|
| `operation` | 操作类型：create/list/delete/metadata | - | 是 |
| `--server` | Kafka 服务器地址 | localhost:9092 | 否 |
| `--topic` | 主题名称 | - | create/delete/metadata 时必需 |
| `--partitions` | 分区数量 | 1 | 否 |
| `--replication-factor` | 副本因子 | 1 | 否 |

## 示例

### 完整工作流程示例

```bash
# 1. 查看当前所有主题
python kafka_manager.py list

# 2. 创建一个新主题
python kafka_manager.py create --topic test-topic --partitions 3 --replication-factor 1

# 3. 验证主题是否创建成功
python kafka_manager.py list

# 4. 查看主题详细信息
python kafka_manager.py metadata --topic test-topic

# 5. 删除主题
python kafka_manager.py delete --topic test-topic
```

## 故障排查

### 运行连接诊断工具

如果遇到连接问题，先运行诊断工具：

```bash
python test_connection.py
```

这会测试：
- TCP 连接是否正常
- Kafka 客户端是否能连接
- 提供详细的错误信息和解决建议

### 常见错误

**NodeNotReadyError**
- 原因：Kafka broker 未准备好或配置问题
- 解决：
  1. 等待几秒后重试
  2. 检查 Kafka 是否完全启动
  3. 验证 `advertised.listeners` 配置

**NoBrokersAvailable**
- 原因：无法连接到任何 Kafka broker
- 解决：
  1. 检查 Kafka 是否运行：`docker ps`
  2. 验证端口 9092 是否开放
  3. 检查防火墙设置

## 注意事项

1. **Kafka 服务器连接**：确保 Kafka 服务器正在运行并且可以访问
2. **副本因子限制**：副本因子不能超过 Kafka 集群中的 broker 数量
3. **主题删除**：删除主题需要 Kafka 配置允许（`delete.topic.enable=true`）
4. **权限**：确保客户端有足够的权限执行相应操作
5. **超时设置**：默认超时 30 秒，如果网络慢可能需要增加

## 错误处理

程序包含完整的错误处理：
- ✗ 主题已存在时创建会提示
- ✗ 主题不存在时删除会提示
- ✗ 连接失败时会显示错误信息

## 开发环境设置

如果你还没有 Kafka 服务器，可以使用 Docker 快速启动：

```bash
# 启动 Zookeeper
docker run -d --name zookeeper -p 2181:2181 zookeeper:3.7

# 启动 Kafka
docker run -d --name kafka -p 9092:9092 \
  -e KAFKA_ZOOKEEPER_CONNECT=localhost:2181 \
  -e KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://localhost:9092 \
  -e KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1 \
  confluentinc/cp-kafka:latest
```

## 许可证

本项目仅供学习和测试使用。
