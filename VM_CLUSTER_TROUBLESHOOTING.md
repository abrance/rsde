# VictoriaMetrics 集群 503 错误排查报告

## 问题描述
VictoriaMetrics 集群在查询时返回 HTTP 503 错误状态码。

## 环境信息
- K3s 单主机集群
- VictoriaMetrics Cluster 版本: v1.128.0-cluster
- 命名空间: bkbase-test

## 组件状态
### Pod 状态
```
vmtest-1-victoria-metrics-cluster-vminsert-6b6694f777-8h6mc   1/1 Running
vmtest-1-victoria-metrics-cluster-vmselect-75b957c95d-hpq7b   1/1 Running  
vmtest-1-victoria-metrics-cluster-vmstorage-0                 1/1 Running
```

### 服务状态
```
vmtest-1-victoria-metrics-cluster-vminsert    ClusterIP 10.43.35.230  8480/TCP
vmtest-1-victoria-metrics-cluster-vmselect    ClusterIP 10.43.21.106  8481/TCP
vmtest-1-victoria-metrics-cluster-vmstorage   ClusterIP None          8482/TCP,8401/TCP,8400/TCP
```

## 错误分析

### 1. 主要错误信息
```
HTTP/1.1 503 Service Unavailable
{"status":"error","errorType":"503","error":"error occured during search: cannot fetch query results from vmstorage nodes: cannot perform search on vmstorage vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8401: cannot obtain connection from a pool: cannot perform \"vmselect\" handshake with server \"vmtest-1-victoria-metrics-cluster-vmstorage-0.vmtest-1-victoria-metrics-cluster-vmstorage.bkbase-test.svc.cluster.local:8401\": cannot read success response after sending hello: cannot read message with size 2: EOF; read only 0 bytes"}
```

### 2. 根本原因
- **vmselect** 无法与 **vmstorage** 建立正确的握手连接
- 错误模式：`cannot read success response after sending hello: cannot read message with size 2: EOF`
- **vminsert** 也有相同的连接问题到 vmstorage:8400

### 3. 网络连接性测试
- TCP 连接正常：`nc -zv vmstorage:8401` 成功
- HTTP 健康检查正常：vmselect `/health` 返回 200
- 问题出现在应用层协议握手

## 已执行的排查步骤

### 1. 检查组件日志
- **vmstorage**: 正常启动，监听正确端口
- **vmselect**: 启动时显示连接成功，但查询时握手失败
- **vminsert**: 连接 vmstorage 时握手错误

### 2. 网络连接测试
- Pod 间网络连通性正常
- 端口监听状态正常
- DNS 解析正常

### 3. 重启组件
- 重启 vmstorage pod 后，初始连接变快但问题依然存在
- 重启所有组件后问题持续

## 可能的解决方案

### 1. 检查组件配置兼容性
```bash
# 检查是否所有组件使用相同的版本和配置
kubectl get pods -n bkbase-test -o jsonpath='{range .items[*]}{.metadata.name}{": "}{.spec.containers[0].image}{"\n"}{end}' | grep victoria
```

### 2. 检查 VictoriaMetrics 集群配置
```bash
# 检查 Helm values 或配置文件
kubectl get configmap -n bkbase-test
kubectl describe configmap -n bkbase-test [configmap-name]
```

### 3. 强制重新创建整个集群
```bash
# 删除所有 VM 相关的 pods，让 StatefulSet/Deployment 重新创建
kubectl delete pods -n bkbase-test -l app.kubernetes.io/name=victoria-metrics-cluster
```

### 4. 检查存储和权限
```bash
# 检查 vmstorage 的存储卷状态
kubectl describe pod -n bkbase-test vmtest-1-victoria-metrics-cluster-vmstorage-0
```

### 5. 版本降级
如果是版本兼容性问题，考虑使用更稳定的版本，如 v1.127.x

## 推荐的立即修复措施

1. **完全重新部署集群**：
   ```bash
   helm uninstall vmtest-1 -n bkbase-test
   # 等待所有资源清理完成
   helm install vmtest-1 victoria-metrics/victoria-metrics-cluster -n bkbase-test --version [stable-version]
   ```

2. **检查 Helm chart 版本兼容性**

3. **验证集群资源配置是否充足**

## 下一步行动
- [ ] 检查 Helm chart 和镜像版本兼容性
- [ ] 完全重新部署集群
- [ ] 配置持久化存储（如果需要数据保留）
- [ ] 验证修复后的集群功能