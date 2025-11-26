# VictoriaMetrics 使用总结

## 环境信息

- **集群**: k3s
- **命名空间**: bkbase-test
- **VM 版本**: v1.128.0

## 可用组件

```bash
kubectl get pods -n bkbase-test | grep vm
```

- `vminsert`: 数据写入接口
- `vmselect`: 数据查询接口
- `vmstorage`: 数据存储

## 快速使用

### 1. 写入数据

```bash
# 使用测试脚本（推荐）
bash /opt/mystorage/github/rsde/doc/vm-test-data.sh

# 手动写入单条数据
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary 'my_metric{label="value"} 123'

# 批量写入
curl -X POST 'http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/import/prometheus' \
  --data-binary @- << 'EOF'
cpu_usage{host="server1"} 45.2
cpu_usage{host="server2"} 38.7
memory_bytes{host="server1"} 2147483648
EOF
```

### 2. 查询数据

```bash
# 查询所有 up 指标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up' | python3 -m json.tool

# 查看所有指标名称
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/label/__name__/values' | python3 -m json.tool

# 查询特定指标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=cpu_usage' | python3 -m json.tool
```

## 文档资源

- **快速参考**: `/opt/mystorage/github/rsde/doc/VM_QUICK_REFERENCE.md`
- **详细指南**: `/opt/mystorage/github/rsde/doc/VICTORIAMETRICS_QUERY_GUIDE.md`
- **测试脚本**: `/opt/mystorage/github/rsde/doc/vm-test-data.sh`

## 服务地址

简化变量：

```bash
export VMINSERT="http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480"
export VMSELECT="http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481"

# 使用示例
curl -X POST "${VMINSERT}/insert/0/prometheus/api/v1/import/prometheus" \
  --data-binary 'test_metric 100'

curl -s "${VMSELECT}/select/0/prometheus/api/v1/query?query=test_metric" | python3 -m json.tool
```

## 常用操作

```bash
# 查看组件状态
kubectl get pods -n bkbase-test | grep vm

# 查看存储的数据量
curl http://vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8482/metrics | grep vm_rows

# 查看组件日志
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vminsert-xxx
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vmselect-xxx
kubectl logs -n bkbase-test vmtest-1-victoria-metrics-cluster-vmstorage-0
```

## 数据格式

Prometheus 文本格式：

```
# HELP metric_name 描述
# TYPE metric_name gauge
metric_name{label1="value1",label2="value2"} 123.45

# 带时间戳（毫秒）
metric_name{label="value"} 123.45 1732156800000
```

## 集成方式

### Grafana

- **数据源类型**: Prometheus
- **URL**: `http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/`

### 其他监控系统

任何支持 Prometheus Remote Write 的系统都可以写入 VM：

```yaml
remote_write:
  - url: http://vmtest-1-victoria-metrics-cluster-vminsert.bkbase-test.svc.cluster.local:8480/insert/0/prometheus/api/v1/write
```

## 注意事项

1. 使用 Prometheus 文本格式手动写入数据
2. 查询使用标准 PromQL 语法
3. 数据保留期：240 个月
4. 通过 CoreDNS 可直接使用集群内域名访问

---

**最后更新**: 2025-11-21
