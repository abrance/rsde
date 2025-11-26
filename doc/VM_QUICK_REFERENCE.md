# VictoriaMetrics 快速参考

## 你的集群信息

**命名空间**: `bkbase-test`

**服务地址**:
- vminsert: `vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480`
- vmselect: `vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481`
- vmstorage: `vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8482`

## 快速开始

### 1. 手动写入数据

```bash
# 使用 Prometheus 文本格式写入
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary @- << 'EOF'
test_metric{job="manual",instance="test"} 42
up{job="test",instance="localhost"} 1
EOF
```

### 2. 查询数据

```bash
# 简单查询
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up'

# 美化输出
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up' | python3 -m json.tool
```

### 3. 运行测试脚本

```bash
# 写入测试数据并查询
bash /opt/mystorage/github/rsde/doc/vm-test-data.sh
```

## 常用 PromQL 查询

```promql
# 查看所有在线服务
up

# 查看特定 job
up{job="api-server"}

# 查看离线服务
up == 0

# HTTP 请求总数
http_requests_total

# 成功的 HTTP 请求
http_requests_total{status="200"}

# 按 method 聚合
sum by (method) (http_requests_total)

# 计算百分比
(count(up == 1) / count(up)) * 100
```

## API 端点

### 查询 API (vmselect)

```bash
BASE_URL="http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1"

# 即时查询
curl "${BASE_URL}/query?query=up"

# 范围查询
curl "${BASE_URL}/query_range?query=up&start=1732156800&end=1732160400&step=60"

# 查询序列
curl "${BASE_URL}/series?match[]=up"

# 查询标签
curl "${BASE_URL}/labels"

# 查询标签值
curl "${BASE_URL}/label/job/values"

# 查询所有指标
curl "${BASE_URL}/label/__name__/values"
```

### 写入 API (vminsert)

```bash
BASE_URL="http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1"

# Prometheus 文本格式
curl -X POST "${BASE_URL}/import/prometheus" --data-binary @metrics.txt

# Remote Write 格式 (需要 protobuf)
curl -X POST "${BASE_URL}/write" --data-binary @metrics.pb
```

## 数据格式示例

### Prometheus 文本格式

```
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",path="/api"} 1234
http_requests_total{method="POST",path="/api"} 567

# HELP cpu_usage CPU usage percentage  
# TYPE cpu_usage gauge
cpu_usage{instance="server1"} 45.2
cpu_usage{instance="server2"} 38.7
```

### 带时间戳的数据

```
metric_name{label="value"} 123.45 1732156800000
```

## 故障排查

```bash
# 检查组件状态
kubectl get pods -n bkbase-test | grep vm

# 查看日志
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vminsert-xxxxx
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vmselect-xxxxx
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vmstorage-0

# 检查存储的数据量
curl http://vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8482/metrics | grep vm_rows

# 测试连通性
curl http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/health
```

## 集成 Grafana

**数据源配置**:
- Type: `Prometheus`
- URL: `http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/`
- Access: `Server`

## Web UI 访问

```bash
# 方式 1: 如果配置了 DNS
http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/vmui/

# 方式 2: 端口转发
kubectl port-forward -n bkbase-test svc/vmtest-1-victoria-metrics-cluster-vmselect 8481:8481
# 然后访问 http://localhost:8481/select/0/vmui/
```

## 下一步

- ✅ 已验证数据写入和查询
- 📊 可以集成 Grafana 可视化
- 🔍 可以通过 Prometheus Remote Write 接入其他数据源

详细教程请查看: `/opt/mystorage/github/rsde/doc/VICTORIAMETRICS_QUERY_GUIDE.md`
