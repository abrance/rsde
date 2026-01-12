# file-transfer-go Helm Chart

用于部署 file-transfer-go 服务的 Helm Chart

## 镜像信息

- **镜像地址**: `swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/matrixseven/file-transfer-go`
- **默认标签**: `latest`
- **命名空间**: `xy`

## 安装

### 前置条件

确保目标命名空间已存在:

```bash
kubectl create namespace xy --dry-run=client -o yaml | kubectl apply -f -
```

### 快速安装

```bash
helm install file-transfer-go ./helm/file-transfer-go -n xy
```

### 使用自定义配置

```bash
helm install file-transfer-go ./helm/file-transfer-go -n xy -f values-custom.yaml
```

### 升级

```bash
helm upgrade file-transfer-go ./helm/file-transfer-go -n xy
```

## 配置说明

### 核心配置

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `namespace` | 部署的命名空间 | `xy` |
| `replicaCount` | 副本数量 | `1` |
| `image.repository` | 镜像仓库地址 | `swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/matrixseven/file-transfer-go` |
| `image.tag` | 镜像标签 | `latest` |
| `image.pullPolicy` | 镜像拉取策略 | `IfNotPresent` |

### Service 配置

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `service.type` | Service 类型 | `ClusterIP` |
| `service.port` | Service 端口 | `8080` |
| `service.targetPort` | 容器端口 | `8080` |

### Ingress 配置

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `ingress.enabled` | 启用 Ingress | `false` |
| `ingress.className` | Ingress 类名 | `traefik` |
| `ingress.annotations` | Ingress 注解 | `{}` |
| `ingress.hosts` | 域名配置 | `[{host: filetransfer.xiaoyxq.top, paths: [{path: /, pathType: Prefix}]}]` |
| `ingress.tls` | TLS 配置 | `[]` |

### 资源配置

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `resources.limits.cpu` | CPU 限制 | `500m` |
| `resources.limits.memory` | 内存限制 | `512Mi` |
| `resources.requests.cpu` | CPU 请求 | `100m` |
| `resources.requests.memory` | 内存请求 | `128Mi` |

### 持久化存储

| 参数 | 描述 | 默认值 |
|------|------|--------|
| `persistence.enabled` | 启用持久化存储 | `false` |
| `persistence.storageClass` | 存储类 | `""` |
| `persistence.accessMode` | 访问模式 | `ReadWriteOnce` |
| `persistence.size` | 存储大小 | `10Gi` |
| `persistence.mountPath` | 挂载路径 | `/data` |

## 启用 Ingress

### 基础配置

在 values.yaml 中启用 Ingress:

```yaml
ingress:
  enabled: true
  className: "traefik"
  hosts:
    - host: filetransfer.xiaoyxq.top
      paths:
        - path: /
          pathType: Prefix
```

### 启用 HTTPS

配置 TLS 证书:

```yaml
ingress:
  enabled: true
  className: "traefik"
  annotations:
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
    traefik.ingress.kubernetes.io/redirect-entry-point: "https"
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

### 部署命令

```bash
# 启用 Ingress 部署
helm upgrade file-transfer-go ./helm/file-transfer-go -n xy \
  --set ingress.enabled=true

# 使用配置文件部署
helm upgrade file-transfer-go ./helm/file-transfer-go -n xy \
  -f values-example.yaml
```

## 卸载

```bash
helm uninstall file-transfer-go -n xy
```

## 示例配置

参考 [values-example.yaml](values-example.yaml) 文件查看更多配置示例
