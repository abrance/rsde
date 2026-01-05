# RSDE Helm Chart

Helm chart for deploying RSDE apiserver with Anybox, OCR and image hosting services.

## 快速开始

### 1. 准备配置文件

复制示例配置并根据您的环境修改：

```bash
cd helm/rsde
cp values-example.yaml values.yaml
```

编辑 `values.yaml`，修改以下关键配置：

- **镜像配置**: `image.repository` 和 `image.tag`
- **Redis 连接**: `config.anybox.redis_url`
- **OCR 认证**: `config.remote_ocr.*` 相关字段
- **存储配置**: `persistence.storageClass` 和 `persistence.size`

> ⚠️ **重要**: `values.yaml` 包含敏感信息，已加入 `.helmignore`，不会被打包或提交到仓库。

### 2. 部署

#### 使用本地 values.yaml（推荐）

```bash
helm install rsde-apiserver ./helm/rsde \
  --namespace xy \
  --create-namespace \
  --values values.yaml
```

#### 使用命令行覆盖（CI/CD 场景）

```bash
helm install rsde-apiserver ./helm/rsde \
  --namespace xy \
  --create-namespace \
  --values values-example.yaml \
  --set image.repository=ghcr.io/abrance/rsde/xy \
  --set image.tag=latest \
  --set config.anybox.redis_url="redis://:password@redis-host:6379/" \
  --set config.remote_ocr.auth_token="your-token" \
  --set config.remote_ocr.auth_uuid="your-uuid" \
  --set config.remote_ocr.auth_cookie="your-cookie"
```

#### 使用外部 values 文件（生产环境）

```bash
# 将生产配置存储在安全位置（如 Kubernetes Secret 或外部配置管理系统）
helm install rsde-apiserver ./helm/rsde \
  --namespace xy \
  --values values-example.yaml \
  --values /secure/path/prod-values.yaml
```

### 3. 升级

```bash
helm upgrade rsde-apiserver ./helm/rsde \
  --namespace xy \
  --values values.yaml \
  --reuse-values
```

### 4. 卸载

```bash
helm uninstall rsde-apiserver --namespace xy
```

## 配置说明

### 必需配置

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `image.repository` | Docker 镜像仓库地址 | `your-registry.com/rsde-apiserver` |
| `image.tag` | 镜像标签 | `v0.1.0` |
| `config.anybox.redis_url` | Redis 连接地址 | `redis://:password@host:6379/0` |

### OCR 配置（可选）

如果需要使用 OCR 功能，需要配置：

| 参数 | 说明 |
|------|------|
| `config.remote_ocr.auth_token` | OCR 服务认证 token |
| `config.remote_ocr.auth_uuid` | OCR 服务认证 UUID |
| `config.remote_ocr.auth_cookie` | OCR 服务认证 Cookie |

### 存储配置

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `persistence.enabled` | 启用持久化存储 | `true` |
| `persistence.storageClass` | 存储类名称 | `""` (使用默认) |
| `persistence.size` | 存储大小 | `20Gi` |

## 依赖服务

### Redis

apiserver 需要 Redis 作为 Anybox 服务的后端存储。您可以：

1. **使用已有 Redis**

   修改 `config.anybox.redis_url` 指向您的 Redis 地址。

2. **部署新的 Redis**

   ```bash
   helm repo add bitnami https://charts.bitnami.com/bitnami
   helm install redis bitnami/redis \
     --namespace xy \
     --set auth.password=your-password \
     --set architecture=standalone
   ```

   然后配置：
   ```yaml
   config:
     anybox:
       redis_url: "redis://:your-password@redis-master.xy.svc.cluster.local/"
   ```

## 健康检查

应用启动后，可以通过以下端点检查状态：

```bash
kubectl exec -n xy deployment/rsde-apiserver -- curl http://localhost:3000/api/anybox/health
```

预期响应：
```json
{"service":"anybox-api","status":"ok","version":"0.1.0"}
```

## 故障排查

### Pod 启动失败

```bash
# 查看 Pod 状态
kubectl get pods -n xy

# 查看日志
kubectl logs -n xy -l app.kubernetes.io/name=rsde-apiserver

# 查看事件
kubectl describe pod -n xy -l app.kubernetes.io/name=rsde-apiserver
```

### 常见问题

1. **ImagePullBackOff**: 检查镜像名称和拉取权限
2. **CrashLoopBackOff**: 检查 Redis 连接配置和日志
3. **PVC Pending**: 检查 StorageClass 是否正确

## 安全建议

1. ✅ **不要提交** `values.yaml` 到版本控制系统
2. ✅ 使用 **Kubernetes Secrets** 管理敏感信息（高级用法）
3. ✅ 在 CI/CD 中使用**环境变量**或**安全存储**传递敏感配置
4. ✅ 定期**轮换** OCR 认证信息
5. ✅ 使用 **RBAC** 限制对配置的访问

## CI/CD 集成

GitHub Actions 示例（已在 `.github/workflows/rsync-ci.yml` 中实现）：

```yaml
- name: Deploy with Helm
  run: |
    helm install rsde-apiserver ./helm/rsde \
      --namespace xy \
      --values helm/rsde/values-example.yaml \
      --set image.repository=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }} \
      --set image.tag=${{ github.sha }} \
      --set config.anybox.redis_url="${{ secrets.REDIS_URL }}" \
      --set config.remote_ocr.auth_token="${{ secrets.OCR_TOKEN }}"
```

使用 GitHub Secrets 存储敏感信息，通过 `--set` 覆盖配置。

## 文件说明

- `values-example.yaml`: 配置模板，包含占位符，提交到仓库
- `values.yaml`: 实际配置，包含敏感信息，已被 `.helmignore` 忽略
- `Chart.yaml`: Chart 元数据
- `.helmignore`: 指定打包时忽略的文件（包括 `values.yaml`）

## 版本

- Chart 版本: 0.1.0
- 应用版本: 0.1.0
