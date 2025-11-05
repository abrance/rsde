#!/usr/bin/env python3
"""
Kafka SASL 认证连接测试脚本
测试带有 SASL/PLAIN 认证的 Kafka 连接
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


def test_kafka_sasl_connection(
    bootstrap_servers="localhost:9092",
    security_protocol="SASL_PLAINTEXT",
    sasl_mechanism="PLAIN",
    sasl_username="admin",
    sasl_password="admin-secret",
):
    """测试带认证的 Kafka 连接"""
    print("=" * 60)
    print("Kafka SASL 认证连接诊断工具")
    print("=" * 60)
    print()

    # 解析服务器地址
    host, port = bootstrap_servers.split(":")
    port = int(port)

    print(f"📍 测试连接: {host}:{port}")
    print(f"🔐 安全协议: {security_protocol}")
    print(f"🔑 SASL 机制: {sasl_mechanism}")
    print(f"👤 用户名: {sasl_username}")
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
        return False

    print()

    # 2. 测试 Kafka SASL 客户端连接
    print("步骤 2: 测试 Kafka SASL 认证连接...")
    try:
        from kafka import KafkaAdminClient

        admin_client = KafkaAdminClient(
            bootstrap_servers=bootstrap_servers,
            client_id="sasl-connection-test",
            security_protocol=security_protocol,
            sasl_mechanism=sasl_mechanism,
            sasl_plain_username=sasl_username,
            sasl_plain_password=sasl_password,
            request_timeout_ms=30000,
            api_version_auto_timeout_ms=30000,
        )

        print(f"✓ Kafka SASL 认证成功!")

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
        print("✓ 所有测试通过! Kafka SASL 连接正常")
        print("=" * 60)
        return True

    except Exception as e:
        print(f"✗ Kafka SASL 连接失败: {e}")
        print()
        print(f"错误类型: {type(e).__name__}")
        print()

        error_name = type(e).__name__

        if "Authentication" in str(e) or "SaslAuthentication" in error_name:
            print("认证失败！")
            print("  可能的原因:")
            print("  1. 用户名或密码错误")
            print("  2. SASL 机制配置不匹配")
            print("  3. Kafka 服务器未启用 SASL 认证")
            print()
            print("  解决方法:")
            print("  1. 检查 kafka_server_jaas.conf 中的用户配置")
            print("  2. 确认使用正确的 SASL 机制")
            print("  3. 重启 Kafka: docker-compose restart kafka")

        elif "NodeNotReadyError" in error_name:
            print("节点未就绪！")
            print("  可能的原因:")
            print("  1. Kafka broker 尚未准备好接受连接")
            print("  2. Kafka 配置的 advertised.listeners 不正确")
            print()
            print("  解决方法:")
            print("  1. 等待几秒后重试")
            print("  2. 检查 Kafka 的配置")

        return False


def print_usage_examples():
    """打印使用示例"""
    print()
    print("=" * 60)
    print("使用 kafka_manager.py 连接带认证的 Kafka")
    print("=" * 60)
    print()
    print("示例命令:")
    print()
    print("# 列出主题")
    print("python kafka_manager.py list \\")
    print("  --security-protocol SASL_PLAINTEXT \\")
    print("  --sasl-mechanism PLAIN \\")
    print("  --sasl-username admin \\")
    print("  --sasl-password admin-secret")
    print()
    print("# 创建主题")
    print("python kafka_manager.py create \\")
    print("  --topic test-topic \\")
    print("  --partitions 3 \\")
    print("  --security-protocol SASL_PLAINTEXT \\")
    print("  --sasl-mechanism PLAIN \\")
    print("  --sasl-username admin \\")
    print("  --sasl-password admin-secret")
    print()
    print("# 删除主题")
    print("python kafka_manager.py delete \\")
    print("  --topic test-topic \\")
    print("  --security-protocol SASL_PLAINTEXT \\")
    print("  --sasl-mechanism PLAIN \\")
    print("  --sasl-username admin \\")
    print("  --sasl-password admin-secret")
    print()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Kafka SASL 认证连接测试工具")
    parser.add_argument(
        "--server",
        default="localhost:9092",
        help="Kafka 服务器地址 (默认: localhost:9092)",
    )
    parser.add_argument(
        "--security-protocol",
        default="SASL_PLAINTEXT",
        help="安全协议 (默认: SASL_PLAINTEXT)",
    )
    parser.add_argument(
        "--sasl-mechanism", default="PLAIN", help="SASL 机制 (默认: PLAIN)"
    )
    parser.add_argument(
        "--sasl-username", default="admin", help="SASL 用户名 (默认: admin)"
    )
    parser.add_argument(
        "--sasl-password", default="admin-secret", help="SASL 密码 (默认: admin-secret)"
    )

    args = parser.parse_args()

    success = test_kafka_sasl_connection(
        bootstrap_servers=args.server,
        security_protocol=args.security_protocol,
        sasl_mechanism=args.sasl_mechanism,
        sasl_username=args.sasl_username,
        sasl_password=args.sasl_password,
    )

    if success:
        print_usage_examples()
        sys.exit(0)
    else:
        print()
        print("建议:")
        print("  1. 检查 Kafka 容器日志: docker-compose logs kafka")
        print("  2. 确认 JAAS 配置文件正确加载")
        print("  3. 验证用户凭证是否正确")
        sys.exit(1)
