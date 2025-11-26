# VictoriaMetrics + vmagent 部署总结

**部署时间**: 2025-11-21  
**集群**: k3s  
**命名空间**: bkbase-test

## ✅ 部署状态

### VictoriaMetrics 集群组件
```
vminsert  (1/1 Running) - 数据写入接口
vmselect  (1/1 Running) - 数据查询接口
vmstorage (1/1 Running) - 数据存储
```

### vmagent 数据采集器
```
vmagent   (1/1 Running) - 自动采集 Kubernetes 指标
```

## 📊 当前状态

- **采集目标**: 8 个（全部在线）
- **采集指标**: 1065+ 个
- **数据保留**: 240 个月

## 🎯 采集范围

### 1. Kubernetes API Server
- API 请求统计
- 审计日志
- 资源使用情况

### 2. Kubernetes Nodes
- 节点资源使用
- kubelet 指标

### 3. Kubernetes Pods (自动发现)
需要 Pod 添加 annotation:
```yaml
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "8080"
```

### 4. Kubernetes Services (自动发现)
需要 Service 添加 annotation:
```yaml
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "8080"
```

### 5. VictoriaMetrics 自身
- vminsert 指标
- vmselect 指标
- vmstorage 指标
- vmagent 指标

## 🔍 快速查询

### 查看所有在线目标
```bash
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=up' | python3 -m json.tool
```

### 查看所有指标
```bash
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/label/__name__/values' | python3 -m json.tool
```

### 查询 Kubernetes API 指标
```bash
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=apiserver_request_total' | python3 -m json.tool
```

### 查看 VM 写入速率
```bash
curl -s 'http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/api/v1/query?query=rate(vm_rows_inserted_total[5m])' | python3 -m json.tool
```

## 🌐 Web UI 访问

### vmselect UI (查询界面)
```bash
# 方式 1: 直接访问（如果配置了 DNS）
http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/vmui/

# 方式 2: 端口转发
kubectl port-forward -n bkbase-test svc/vmtest-1-victoria-metrics-cluster-vmselect 8481:8481
# 访问 http://localhost:8481/select/0/vmui/
```

### vmagent UI (采集状态)
```bash
kubectl port-forward -n bkbase-test svc/vmagent 8429:8429
# 访问 http://localhost:8429/targets
```

## 📁 相关文件

### 配置文件
- `/opt/mystorage/github/rsde/doc/vmagent-deployment.yaml` - vmagent 部署配置

### 文档
- `/opt/mystorage/github/rsde/doc/VMAGENT_GUIDE.md` - vmagent 详细指南
- `/opt/mystorage/github/rsde/doc/VICTORIAMETRICS_QUERY_GUIDE.md` - VM 查询指南
- `/opt/mystorage/github/rsde/doc/VM_QUICK_REFERENCE.md` - VM 快速参考
- `/opt/mystorage/github/rsde/doc/VM_USAGE_SUMMARY.md` - VM 使用总结

### 脚本
- `/opt/mystorage/github/rsde/doc/vm-test-data.sh` - 测试数据写入脚本

## 🔧 常用命令

### 查看组件状态
```bash
kubectl get pods -n bkbase-test | grep -E "vm|vmagent"
```

### 查看 vmagent 日志
```bash
kubectl logs -n bkbase-test -l app=vmagent --tail=50
```

### 查看采集目标
```bash
curl -s 'http://vmagent.bkbase-test.svc.cluster.local:8429/api/v1/targets' | python3 -m json.tool
```

### 重启 vmagent
```bash
kubectl rollout restart deployment vmagent -n bkbase-test
```

### 更新 vmagent 配置
```bash
kubectl edit configmap vmagent-config -n bkbase-test
kubectl rollout restart deployment vmagent -n bkbase-test
```

## 📈 Grafana 集成

### 添加数据源

1. **类型**: Prometheus
2. **URL**: `http://vmtest-1-victoria-metrics-cluster-vmselect.bkbase-test.svc.cluster.local:8481/select/0/prometheus/`
3. **Access**: Server (如果 Grafana 在同一集群)

### 推荐的仪表板

可以导入以下 Grafana 仪表板 ID:
- **14205**: VictoriaMetrics - cluster
- **14516**: VictoriaMetrics - vmagent
- **11074**: Node Exporter Full (如果采集了 node-exporter)
- **7249**: Kubernetes Cluster Monitoring

## 🎨 使用场景

### 1. 监控 Kubernetes 集群
所有 Kubernetes 组件指标已自动采集

### 2. 监控自定义应用
为你的应用 Pod 添加 annotation 即可自动采集

### 3. 监控 VictoriaMetrics 自身
VM 组件的健康状态和性能指标

### 4. 手动写入数据
依然可以通过 curl 手动写入测试数据

## ⚠️ 注意事项

1. **Node 采集可能失败**: 由于 RBAC 权限限制，节点代理采集可能需要额外配置
2. **重复目标警告**: 某些 Pod 可能暴露多个端口，导致重复采集警告（可忽略）
3. **内存使用**: 采集目标增多时，可能需要调整 vmagent 的内存限制

## 🚀 下一步建议

1. ✅ 配置 Grafana 数据源和仪表板
2. 📊 为你的应用添加 prometheus.io annotation
3. 🔍 创建自定义告警规则
4. 📈 配置数据备份策略

## 🛠️ 故障排查

### vmagent 未采集数据
```bash
# 检查日志
kubectl logs -n bkbase-test -l app=vmagent

# 检查采集目标
curl http://vmagent.bkbase-test.svc.cluster.local:8429/api/v1/targets
```

### 查询不到数据
```bash
# 检查数据是否写入
curl http://vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8482/metrics | grep vm_rows

# 检查 vminsert 日志
kubectl logs -n bkbase-test -l app.kubernetes.io/component=vminsert
```

## 📚 参考资源

- [VictoriaMetrics 官方文档](https://docs.victoriametrics.com/)
- [vmagent 文档](https://docs.victoriametrics.com/vmagent.html)
- [PromQL 教程](https://prometheus.io/docs/prometheus/latest/querying/basics/)

---

**部署完成！** 🎉

你现在拥有一个功能完整的 Kubernetes 监控系统，可以自动采集集群指标并使用 PromQL 进行查询。
