#!/usr/bin/env python3
"""
Kafka Connection Test Script
测试 Kafka 连接并提供诊断信息
"""

import socket
import sys


def test_tcp_connection(host, port):
    """测试 TCP 连接"""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        result = sock.connect_ex((host, port))
        sock.close()
        return result == 0
    except Exception as e:
        print(f"✗ TCP 连接测试失败: {e}")
        return False


def test_kafka_connection(bootstrap_servers="localhost:9092"):
    """测试 Kafka 连接"""
    print("=" * 60)
    print("Kafka 连接诊断工具")
    print("=" * 60)
    print()

    # 解析服务器地址
    host, port = bootstrap_servers.split(":")
    port = int(port)

    print(f"📍 测试连接: {host}:{port}")
    print()

    # 1. 测试 TCP 连接
    print("步骤 1: 测试 TCP 连接...")
    if test_tcp_connection(host, port):
        print(f"✓ TCP 连接成功: {host}:{port}")
    else:
        print(f"✗ TCP 连接失败: {host}:{port}")
        print()
        print("可能的原因:")
        print("  1. Kafka 服务器未启动")
        print("  2. 防火墙阻止了连接")
        print("  3. 地址或端口配置错误")
        print()
        print("建议:")
        print("  - 检查 Kafka 是否正在运行")
        print("  - 使用 Docker 启动 Kafka (见下方说明)")
        print("  - 检查防火墙设置")
        return False

    print()

    # 2. 测试 Kafka 客户端连接
    print("步骤 2: 测试 Kafka 客户端连接...")
    try:
        from kafka import KafkaAdminClient
        from kafka.errors import KafkaError

        admin_client = KafkaAdminClient(
            bootstrap_servers=bootstrap_servers,
            client_id="connection-test",
            request_timeout_ms=10000,
            api_version_auto_timeout_ms=10000,
        )

        print(f"✓ Kafka 客户端连接成功!")

        # 3. 尝试列出主题
        print()
        print("步骤 3: 测试主题列表功能...")
        topics = admin_client.list_topics()
        print(f"✓ 成功获取主题列表 (共 {len(topics)} 个主题)")

        if topics:
            print("\n当前主题:")
            for topic in sorted(topics):
                print(f"  - {topic}")

        admin_client.close()

        print()
        print("=" * 60)
        print("✓ 所有测试通过! Kafka 连接正常")
        print("=" * 60)
        return True

    except Exception as e:
        print(f"✗ Kafka 客户端连接失败: {e}")
        print()
        print(f"错误类型: {type(e).__name__}")
        print()

        if "NodeNotReadyError" in str(type(e).__name__):
            print("这个错误通常表示:")
            print("  1. Kafka broker 尚未准备好接受连接")
            print("  2. Kafka 配置的 advertised.listeners 不正确")
            print("  3. 网络延迟或超时")
            print()
            print("解决方法:")
            print("  1. 等待几秒后重试")
            print("  2. 检查 Kafka 的 server.properties 配置")
            print("  3. 增加超时时间")

        return False


def print_docker_help():
    """打印 Docker 启动 Kafka 的帮助信息"""
    print()
    print("=" * 60)
    print("如何使用 Docker 启动 Kafka")
    print("=" * 60)
    print()
    print("方法 1: 使用 Docker Compose (推荐)")
    print("-" * 60)
    print(
        """
创建 docker-compose.yml 文件:

version: '3'
services:
  zookeeper:
    image: confluentinc/cp-zookeeper:latest
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181
      ZOOKEEPER_TICK_TIME: 2000
    ports:
      - "2181:2181"

  kafka:
    image: confluentinc/cp-kafka:latest
    depends_on:
      - zookeeper
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_LISTENER_SECURITY_PROTOCOL_MAP: PLAINTEXT:PLAINTEXT
      KAFKA_INTER_BROKER_LISTENER_NAME: PLAINTEXT
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1

然后运行:
  docker-compose up -d
    """
    )

    print()
    print("方法 2: 使用单个 Docker 命令")
    print("-" * 60)
    print(
        """
# 1. 启动 Zookeeper
docker run -d --name zookeeper \\
  -p 2181:2181 \\
  confluentinc/cp-zookeeper:latest \\
  -e ZOOKEEPER_CLIENT_PORT=2181

# 2. 启动 Kafka
docker run -d --name kafka \\
  -p 9092:9092 \\
  --link zookeeper \\
  -e KAFKA_ZOOKEEPER_CONNECT=zookeeper:2181 \\
  -e KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://localhost:9092 \\
  -e KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1 \\
  confluentinc/cp-kafka:latest
    """
    )

    print()
    print("方法 3: 使用 Redpanda (轻量级替代)")
    print("-" * 60)
    print(
        """
docker run -d --name redpanda \\
  -p 9092:9092 \\
  docker.redpanda.com/vectorized/redpanda:latest \\
  redpanda start --smp 1 --memory 1G \\
  --kafka-addr PLAINTEXT://0.0.0.0:9092 \\
  --advertise-kafka-addr PLAINTEXT://localhost:9092
    """
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Kafka 连接测试工具")
    parser.add_argument(
        "--server",
        default="localhost:9092",
        help="Kafka 服务器地址 (默认: localhost:9092)",
    )
    parser.add_argument(
        "--help-docker", action="store_true", help="显示 Docker 启动 Kafka 的帮助信息"
    )

    args = parser.parse_args()

    if args.help_docker:
        print_docker_help()
        sys.exit(0)

    success = test_kafka_connection(args.server)

    if not success:
        print_docker_help()
        sys.exit(1)
