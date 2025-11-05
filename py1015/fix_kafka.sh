#!/bin/bash
# Kafka Docker 连接问题一键修复脚本

set -e

echo "============================================================"
echo "Kafka Docker 连接问题修复工具"
echo "============================================================"
echo ""

KAFKA_DIR="/opt/mystorage/github/tools/deploy/kafka"
PY_DIR="/opt/mystorage/github/rsde/py1015"

# 检查 Kafka 目录
if [ ! -d "$KAFKA_DIR" ]; then
    echo "✗ Kafka 目录不存在: $KAFKA_DIR"
    exit 1
fi

echo "✓ 找到 Kafka 目录: $KAFKA_DIR"
echo ""

# 获取本机 IP
MY_IP=$(hostname -I | awk '{print $1}')
echo "🌐 检测到本机 IP: $MY_IP"
echo ""

# 显示当前配置
echo "📋 当前 Kafka ADVERTISED_LISTENERS 配置:"
docker exec kafka-kafka-1 env | grep KAFKA_ADVERTISED_LISTENERS || echo "  (无法获取，容器可能未运行)"
echo ""

# 选择修复方案
echo "请选择修复方案:"
echo ""
echo "  1) 使用本机 IP ($MY_IP) - 推荐，支持其他机器访问"
echo "  2) 使用 localhost - 简单，仅本机访问"
echo "  3) 取消"
echo ""
read -p "请选择 [1-3]: " choice

case $choice in
    1)
        echo ""
        echo "使用方案 1: 设置 IP=$MY_IP"
        echo ""
        
        # 创建 .env 文件
        cd "$KAFKA_DIR"
        echo "IP=$MY_IP" > .env
        echo "✓ 创建 .env 文件:"
        cat .env
        echo ""
        
        # 重启 Kafka
        echo "🔄 重启 Kafka..."
        docker-compose down
        docker-compose up -d
        ;;
        
    2)
        echo ""
        echo "使用方案 2: 设置为 localhost"
        echo ""
        
        # 创建 .env 文件
        cd "$KAFKA_DIR"
        echo "IP=localhost" > .env
        echo "✓ 创建 .env 文件:"
        cat .env
        echo ""
        
        # 重启 Kafka
        echo "🔄 重启 Kafka..."
        docker-compose down
        docker-compose up -d
        ;;
        
    3)
        echo "取消操作"
        exit 0
        ;;
        
    *)
        echo "✗ 无效选择"
        exit 1
        ;;
esac

# 等待 Kafka 启动
echo ""
echo "⏳ 等待 Kafka 启动 (30秒)..."
for i in {1..30}; do
    echo -n "."
    sleep 1
done
echo ""
echo ""

# 验证配置
echo "🔍 验证新配置:"
docker exec kafka-kafka-1 env | grep KAFKA_ADVERTISED_LISTENERS
echo ""

# 测试连接
echo "🧪 测试连接..."
cd "$PY_DIR"
python test_connection.py

echo ""
echo "============================================================"
echo "✅ 修复完成！"
echo "============================================================"
echo ""
echo "现在可以使用以下命令:"
echo "  cd $PY_DIR"
echo "  python kafka_manager.py list"
echo "  python kafka_manager.py create --topic test-topic"
echo ""
