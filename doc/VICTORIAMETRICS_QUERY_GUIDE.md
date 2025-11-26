# VictoriaMetrics 查询指南

## 集群架构

你的 VictoriaMetrics 集群组件：

```bash
# 组件状态
kubectl get pods -n bkbase-test | grep vm

# vminsert: 负责接收数据写入
# vmselect: 负责处理查询请求  
# vmstorage: 负责数据存储
```

## 数据流向

```
数据源 → vminsert (写入) → vmstorage (存储) → vmselect (查询)
```

## 手动写入数据

使用 curl 向 vminsert 写入数据：

```bash
# 使用 Prometheus 文本格式写入
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary @- << 'EOF'
test_metric{job="manual",instance="test"} 42
up{job="test",instance="localhost"} 1
http_requests_total{method="GET",status="200"} 1234
EOF

# 或从文件写入
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary @metrics.txt
```

## 查询方式

### 1. 通过 vmselect 服务查询

vmselect 服务地址：
```
http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481
```

### 2. 基础查询示例

#### 即时查询
```bash
# 查询所有 up 指标
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up'

# 查询特定 job 的 up 状态
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up{job="victoriametrics"}'

# 查询 VictoriaMetrics 自身指标
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=vm_rows'
```

#### 范围查询
```bash
# 查询最近 1 小时的数据
curl -G 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query_range' \
  --data-urlencode 'query=up' \
  --data-urlencode 'start='$(date -u -d '1 hour ago' +%s) \
  --data-urlencode 'end='$(date -u +%s) \
  --data-urlencode 'step=1m'
```

#### 元数据查询
```bash
# 获取所有指标名称
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/label/__name__/values'

# 获取所有 job 标签值
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/label/job/values'

# 查询所有序列
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/series?match[]=up'
```

### 3. 常用 PromQL 示例

#### 基础查询
```promql
# 查看所有在线的目标
up

# 查看特定组件
up{component="victoriametrics"}

# 查看特定命名空间
up{kubernetes_namespace="bkbase-test"}
```

#### 聚合查询
```promql
# 按 job 聚合
sum by (job) (up)

# 计算在线率
avg(up) * 100

# 按命名空间统计 pod 数量
count by (kubernetes_namespace) (up)
```

#### 速率计算
```promql
# HTTP 请求速率
rate(vm_http_requests_total[5m])

# 数据写入速率
rate(vm_rows_inserted_total[5m])

# 查询速率
rate(vm_http_requests_total{path="/select/0/prometheus/api/v1/query"}[5m])
```

#### VictoriaMetrics 监控指标
```promql
# 存储的时间序列数量
vm_rows

# 缓存命中率
rate(vm_cache_requests_total[5m]) - rate(vm_cache_misses_total[5m])

# 磁盘使用
vm_data_size_bytes

# 内存使用
process_resident_memory_bytes

# 活跃时间序列
vm_active_timeseries
```

#### Kubernetes 集群监控
```promql
# Node 数量
count(up{job="kubernetes-nodes"})

# Pod 总数
count(up{job="kubernetes-pods"})

# 不健康的 Pod
count(up{job="kubernetes-pods"} == 0)
```

### 4. 使用简写域名（需配置 DNS）

如果你已经配置了 CoreDNS，可以使用简写：

```bash
# 完整域名
curl 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up'

# 或者配置 /etc/hosts 简化
# 127.0.0.1 vmselect
# kubectl port-forward -n bkbase-test svc/vmtest-1-victoria-metrics-cluster-vmselect 8481:8481
```

### 5. 使用 vmui (Web 界面)

VictoriaMetrics 自带 Web UI：

```bash
# 访问 vmselect 的 UI
http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/vmui/

# 或通过端口转发
kubectl port-forward -n bkbase-test svc/vmtest-1-victoria-metrics-cluster-vmselect 8481:8481
# 然后访问 http://localhost:8481/select/0/vmui/
```

### 6. 集成 Grafana

在 Grafana 中添加数据源：

- **类型**: Prometheus
- **URL**: `http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/`
- **Access**: Server (如果 Grafana 在同一集群)

示例仪表板查询：

```promql
# CPU 使用率
rate(process_cpu_seconds_total{job="victoriametrics"}[5m]) * 100

# 内存使用
process_resident_memory_bytes{job="victoriametrics"} / 1024 / 1024

# 写入速率
sum(rate(vm_rows_inserted_total[5m])) by (type)

# 查询延迟
histogram_quantile(0.99, sum(rate(vm_request_duration_seconds_bucket[5m])) by (le))
```

## 故障排查

### 查询返回空结果

```bash
# 1. 检查组件运行状态
kubectl get pods -n bkbase-test | grep vm

# 2. 检查 vminsert 日志
kubectl logs -n bkbase-test -l app.kubernetes.io/component=vminsert

# 3. 查看 vmstorage 存储的数据量
curl http://vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8482/metrics | grep vm_rows

# 4. 手动写入测试数据
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary 'test_metric 123'
```

### 数据保留时间

检查配置的保留期：

```bash
# 当前配置是 240 个月（从日志可见）
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vmstorage-0 | grep retentionPeriod
```

## API 端点参考

### vmselect 端点

- `/select/0/prometheus/api/v1/query` - 即时查询
- `/select/0/prometheus/api/v1/query_range` - 范围查询
- `/select/0/prometheus/api/v1/series` - 序列查询
- `/select/0/prometheus/api/v1/labels` - 标签列表
- `/select/0/prometheus/api/v1/label/<name>/values` - 标签值
- `/select/0/vmui/` - Web UI

### vminsert 端点

- `/insert/0/prometheus/api/v1/write` - 接收远程写入（Prometheus Remote Write 协议）
- `/insert/0/prometheus/api/v1/import/prometheus` - 导入 Prometheus 文本格式数据

## 性能优化建议

1. **使用标签过滤**减少查询范围
2. **避免高基数标签**（如 UUID、IP 地址）
3. **合理控制数据写入频率**
4. **监控 VM 自身指标**确保健康运行

## 测试脚本

运行测试脚本写入示例数据：

```bash
bash /opt/mystorage/github/rsde/doc/vm-test-data.sh
```

## 更多资源

- [VictoriaMetrics 文档](https://docs.victoriametrics.com/)
- [PromQL 教程](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [VM HTTP API](https://docs.victoriametrics.com/Single-server-VictoriaMetrics.html#how-to-import-data-in-prometheus-exposition-format)
