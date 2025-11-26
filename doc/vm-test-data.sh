#!/bin/bash

# VictoriaMetrics 手动写入测试数据脚本

VMINSERT_URL="http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480"
VMSELECT_URL="http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481"

echo "=== VictoriaMetrics 数据写入和查询演示 ==="
echo ""

# 1. 写入测试数据
echo "1. 写入测试数据..."
curl -X POST "${VMINSERT_URL}/insert/0/prometheus/api/v1/import/prometheus" \
  --data-binary @- << 'EOF'
# HTTP 请求指标
http_requests_total{method="GET",path="/api/users",status="200"} 1543
http_requests_total{method="POST",path="/api/users",status="201"} 234
http_requests_total{method="GET",path="/api/products",status="200"} 3421
http_requests_total{method="DELETE",path="/api/users",status="204"} 12

# 服务健康状态
up{job="api-server",instance="api-1"} 1
up{job="api-server",instance="api-2"} 1
up{job="database",instance="db-1"} 1
up{job="cache",instance="redis-1"} 0

# CPU 使用率
cpu_usage_percent{instance="api-1"} 45.2
cpu_usage_percent{instance="api-2"} 38.7
cpu_usage_percent{instance="db-1"} 72.3

# 内存使用
memory_usage_bytes{instance="api-1"} 2147483648
memory_usage_bytes{instance="api-2"} 1879048192
memory_usage_bytes{instance="db-1"} 4294967296

# 自定义业务指标
order_count{status="completed"} 456
order_count{status="pending"} 23
order_count{status="cancelled"} 12
user_online_count 1234
EOF

echo ""
echo "✓ 数据写入完成"
echo ""

# 等待数据可查询
sleep 2

# 2. 查询示例
echo "2. 执行查询示例..."
echo ""

echo "--- 查询所有服务状态 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/query?query=up" | python3 -m json.tool
echo ""

echo "--- 查询所有指标名称 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/label/__name__/values" | python3 -m json.tool
echo ""

echo "--- 查询 HTTP 请求总数 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/query?query=http_requests_total" | python3 -m json.tool
echo ""

echo "--- 查询成功的 HTTP 请求 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/query?query=http_requests_total{status=\"200\"}" | python3 -m json.tool
echo ""

echo "--- 查询不健康的服务 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/query?query=up==0" | python3 -m json.tool
echo ""

echo "--- 查询所有 job 标签 ---"
curl -s "${VMSELECT_URL}/select/0/prometheus/api/v1/label/job/values" | python3 -m json.tool
echo ""

echo "=== 演示完成 ==="
echo ""
echo "提示："
echo "- vminsert 写入端点: ${VMINSERT_URL}/insert/0/prometheus/api/v1/import/prometheus"
echo "- vmselect 查询端点: ${VMSELECT_URL}/select/0/prometheus/api/v1/query"
echo "- Web UI: ${VMSELECT_URL}/select/0/vmui/"
echo ""
