# vmagent 部署和使用指南

## 部署状态

✅ **vmagent 已成功部署并运行**

```bash
kubectl get pods -n bkbase-test -l app=vmagent
```

## 架构说明

```
┌─────────────┐
│  vmagent    │ ← 自动发现并采集 Kubernetes 集群指标
└──────┬──────┘
       │
       ↓ Remote Write
┌─────────────┐
│  vminsert   │ ← 接收数据
└──────┬──────┘
       │
       ↓
┌─────────────┐
│  vmstorage  │ ← 存储数据
└──────┬──────┘
       │
       ↓ 查询
┌─────────────┐
│  vmselect   │ ← 提供查询接口
└─────────────┘
```

## 自动采集的指标

vmagent 已配置以下采集任务：

### 1. kubernetes-apiservers
采集 Kubernetes API 服务器指标

### 2. kubernetes-nodes
采集节点指标（kubelet metrics）

### 3. kubernetes-pods
自动发现并采集带有特定 annotation 的 Pod 指标

**如何启用 Pod 指标采集**：

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-app
  annotations:
    prometheus.io/scrape: "true"   # 启用采集
    prometheus.io/port: "8080"     # 指标端口
    prometheus.io/path: "/metrics" # 指标路径（默认 /metrics）
spec:
  containers:
  - name: app
    image: my-app:latest
    ports:
    - containerPort: 8080
```

### 4. kubernetes-services
采集带有 prometheus.io/scrape annotation 的 Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: my-service
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "8080"
    prometheus.io/path: "/metrics"
spec:
  selector:
    app: my-app
  ports:
  - port: 8080
```

### 5. victoriametrics
采集 VictoriaMetrics 集群自身的指标

## 检查 vmagent 状态

### 查看运行状态
```bash
kubectl get pods -n bkbase-test -l app=vmagent
kubectl logs -n bkbase-test -l app=vmagent --tail=50
```

### 查看采集目标
```bash
# 通过 API 查看
curl -s 'http://vmagent.bkbase-test.svc.cluster.local:8429/api/v1/targets' | python3 -m json.tool

# 在浏览器中查看（需要端口转发）
kubectl port-forward -n bkbase-test svc/vmagent 8429:8429
# 访问 http://localhost:8429/targets
```

### 查看 vmagent 指标
```bash
curl 'http://vmagent.bkbase-test.svc.cluster.local:8429/metrics'

# 查看采集统计
curl 'http://vmagent.bkbase-test.svc.cluster.local:8429/metrics' | grep vm_promscrape_targets

# 查看写入统计
curl 'http://vmagent.bkbase-test.svc.cluster.local:8429/metrics' | grep vm_rows_inserted_total
```

## 验证数据采集

### 1. 查询采集的数据
```bash
# 查询所有在线的目标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up' | python3 -m json.tool

# 查看所有指标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/label/__name__/values' | python3 -m json.tool

# 查询 Kubernetes API 指标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=apiserver_request_total' | python3 -m json.tool
```

### 2. 常用查询示例

```bash
# 查看各个 job 的目标数
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=count by (job) (up)' | python3 -m json.tool

# 查看不健康的目标
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up==0' | python3 -m json.tool

# 查询 VictoriaMetrics 写入速率
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=rate(vm_rows_inserted_total[5m])' | python3 -m json.tool
```

## 配置说明

### 采集间隔
默认配置：
- `scrape_interval: 30s` - 每 30 秒采集一次
- `scrape_timeout: 10s` - 采集超时时间 10 秒

### 修改配置
编辑 ConfigMap 后重启 vmagent：
```bash
kubectl edit configmap vmagent-config -n bkbase-test
kubectl rollout restart deployment vmagent -n bkbase-test
```

### 资源限制
当前配置：
- CPU: 100m (request) - 500m (limit)
- Memory: 256Mi (request) - 512Mi (limit)

## 故障排查

### 采集目标不显示
```bash
# 1. 检查 Pod/Service 的 annotation
kubectl get pod <pod-name> -o yaml | grep -A 5 annotations

# 2. 检查 vmagent 日志
kubectl logs -n bkbase-test -l app=vmagent | grep -i error

# 3. 检查服务发现
curl 'http://vmagent.bkbase-test.svc.cluster.local:8429/api/v1/targets' | python3 -m json.tool
```

### 指标未写入 VictoriaMetrics
```bash
# 检查 vmagent 到 vminsert 的连接
kubectl logs -n bkbase-test -l app=vmagent | grep remoteWrite

# 检查 vminsert 日志
kubectl logs -n bkbase-test -l app.kubernetes.io/component=vminsert
```

### 权限问题
```bash
# 检查 RBAC 权限
kubectl get clusterrole vmagent -o yaml
kubectl get clusterrolebinding vmagent -o yaml
```

## 监控 vmagent 自身

vmagent 自身也暴露指标（已配置 annotation），会被自动采集：

```promql
# vmagent 内存使用
process_resident_memory_bytes{job="kubernetes-pods",kubernetes_pod_name=~"vmagent.*"}

# vmagent CPU 使用
rate(process_cpu_seconds_total{job="kubernetes-pods",kubernetes_pod_name=~"vmagent.*"}[5m])

# 采集目标数量
vm_promscrape_targets{job="kubernetes-pods",kubernetes_pod_name=~"vmagent.*"}

# 采集失败数
rate(vm_promscrape_scrapes_failed_total{job="kubernetes-pods",kubernetes_pod_name=~"vmagent.*"}[5m])
```

## 扩展采集

### 添加静态目标

编辑 ConfigMap 添加新的 scrape_configs：

```yaml
scrape_configs:
  - job_name: 'my-custom-app'
    static_configs:
      - targets:
        - 'app1.default.svc:8080'
        - 'app2.default.svc:8080'
        labels:
          environment: production
```

### 添加自定义服务发现

参考 [vmagent 文档](https://docs.victoriametrics.com/vmagent.html) 配置更多服务发现机制。

## Web UI

vmagent 提供 Web UI 查看采集状态：

```bash
# 端口转发
kubectl port-forward -n bkbase-test svc/vmagent 8429:8429

# 访问以下 URL:
# - http://localhost:8429/ - 主页
# - http://localhost:8429/targets - 采集目标
# - http://localhost:8429/metrics - vmagent 指标
# - http://localhost:8429/config - 配置信息
```

## 性能调优

### 增加并发采集
修改 Deployment args：
```yaml
args:
  - -promscrape.maxScrapeSize=32MB  # 增加最大采集大小
  - -promscrape.discovery.concurrency=2  # 增加服务发现并发
```

### 调整内存限制
```yaml
resources:
  limits:
    memory: 1Gi  # 根据采集目标数量调整
```

## 卸载

```bash
kubectl delete -f /opt/mystorage/github/rsde/doc/vmagent-deployment.yaml
```

## 相关文档

- vmagent 配置文件: `/opt/mystorage/github/rsde/doc/vmagent-deployment.yaml`
- VictoriaMetrics 查询指南: `/opt/mystorage/github/rsde/doc/VICTORIAMETRICS_QUERY_GUIDE.md`
- VM 快速参考: `/opt/mystorage/github/rsde/doc/VM_QUICK_REFERENCE.md`

---

**部署时间**: 2025-11-21  
**vmagent 版本**: v1.128.0  
**镜像**: swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/victoriametrics/vmagent:v1.128.0
