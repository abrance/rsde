# file-transfer-go 部署方案

基于 Helm 的 file-transfer-go 服务部署方案

## 目录结构

```
k8s/file-transfer-go/
├── deploy.sh                    # 一键部署脚本 (含 Ingress 配置)
└── helm/
    └── file-transfer-go/       # Helm Chart
        ├── Chart.yaml          # Chart 元数据
        ├── values.yaml         # 默认配置
        ├── values-example.yaml # 示例配置
        ├── README.md           # Chart 文档
        ├── .helmignore         # 忽略文件
        └── templates/          # K8s 资源模板
            ├── _helpers.tpl    # 辅助模板
            ├── deployment.yaml # 部署配置
            ├── service.yaml    # 服务配置
            ├── ingress.yaml    # Ingress 配置
            └── pvc.yaml        # 持久化存储
```

## 快速开始

### 使用一键部署脚本

```bash
cd k8s/file-transfer-go

# 默认部署并启用 Ingress
./deploy.sh

# 只部署不启用 Ingress
ENABLE_INGRESS=false ./deploy.sh
```

脚本会自动执行以下操作:
- 检查 helm 和 kubectl 环境
- 创建 xy 命名空间(如不存在)
- 部署或升级 file-transfer-go 服务
- 启用 Ingress (默认)
- 验证部署状态

注意: 命名空间由部署脚本管理，不由 Helm 管理

### 手动部署

```bash
cd k8s/file-transfer-go

# 确保命名空间存在
kubectl create namespace xy --dry-run=client -o yaml | kubectl apply -f -

# 部署到 xy 命名空间
helm install file-transfer-go ./helm/file-transfer-go -n xy

# 使用自定义配置
helm install file-transfer-go ./helm/file-transfer-go -n xy -f custom-values.yaml
```

## 配置说明

### 基础配置

编辑 `helm/file-transfer-go/values.yaml` 文件进行配置:

```yaml
namespace: xy
replicaCount: 1

image:
  repository: swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/matrixseven/file-transfer-go
  tag: "latest"
  pullPolicy: IfNotPresent
```

### 服务配置

```yaml
service:
  type: ClusterIP    # 可选: ClusterIP, NodePort, LoadBalancer
  port: 8080
  targetPort: 8080
```

### Ingress 配置

启用 Ingress 暴露服务:

```yaml
ingress:
  enabled: true
  className: "traefik"
  hosts:
    - host: filetransfer.xiaoyxq.top
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: filetransfer-tls
      hosts:
        - filetransfer.xiaoyxq.top
```

### 资源限制

```yaml
resources:
  limits:
    cpu: 500m
    memory: 512Mi
  requests:
    cpu: 100m
    memory: 128Mi
```

### 持久化存储

如需启用持久化存储:

```yaml
persistence:
  enabled: true
  storageClass: "local-path"
  accessMode: ReadWriteOnce
  size: 10Gi
  mountPath: /data
```

## 运维操作

### 查看状态

```bash
# 查看 pod 状态
kubectl get pods -n xy -l app.kubernetes.io/name=file-transfer-go

# 查看服务
kubectl get svc -n xy

# 查看 Ingress
kubectl get ingress -n xy

# 查看日志
kubectl logs -f -n xy -l app.kubernetes.io/name=file-transfer-go
```
部署时启用 Ingress (默认行为)
./deploy.sh

# 或使用环境变量显式启用
ENABLE_INGRESS=true ./deploy.sh

# 手动使用 helm 命令
cd k8s/file-transfer-go

# 方式一: 使用脚本快速启用
./enable-ingress.sh

# 方式二: 使用 helm 命令启用
helm upgrade file-transfer-go ./helm/file-transfer-go -n xy \
  --set ingress.enabled=true

# 验证 Ingress
kubectl get ingress -n xy
kubectl describe ingress file-transfer-go -n xy
```

访问地址: `http://filetransfer.xiaoyxq.top`

注意: 需要将域名 `filetransfer.xiaoyxq.top` 解析到 Ingress Controller 的 IP 地址

### 升级服务

```bash
cd k8s/file-transfer-go
helm upgrade file-transfer-go ./helm/file-transfer-go -n xy
```

### 回滚版本

```bash
# 查看历史版本
helm history file-transfer-go -n xy

# 回滚到上一版本
helm rollback file-transfer-go -n xy

# 回滚到指定版本
helm rollback file-transfer-go <revision> -n xy
```

### 卸载服务

```bash
helm uninstall file-transfer-go -n xy

# 如需删除命名空间
kubectl delete namespace xy
```

## 健康检查

服务包含以下健康检查:

- **Liveness Probe**: 检查路径 `/health`, 30秒后开始, 每10秒检查一次
- **Readiness Probe**: 检查路径 `/health`, 10秒后开始, 每5秒检查一次

## 注意事项

- 确保 Kubernetes 集群可访问华为云镜像仓库 `swr.cn-north-4.myhuaweicloud.com`
- 默认部署在 `xy` 命名空间, 可通过修改 values.yaml 调整
- 健康检查路径假设为 `/health`, 如实际不同需修改配置
- 默认端口为 8080, 请根据实际应用调整
- Ingress 使用 Traefik 控制器, 与 rsde-apiserver 保持一致

## 故障排查

### Pod 启动失败

```bash
# 查看 pod 详情
kubectl describe pod -n xy -l app.kubernetes.io/name=file-transfer-go

# 查看日志
kubectl logs -n xy -l app.kubernetes.io/name=file-transfer-go
```

### 镜像拉取失败

检查镜像仓库访问权限:

```bash
kubectl get events -n xy --sort-by='.lastTimestamp'
```

### 服务无法访问

```bash
# 检查 service
kubectl get svc -n xy

# 检查 endpoints
kubectl get endpoints -n xy
```
