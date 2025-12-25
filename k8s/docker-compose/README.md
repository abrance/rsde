# RSDE API Server Docker Compose

这是用于快速启动 RSDE API Server 的 Docker Compose 配置。

## 前置条件

1. 已构建 Docker 镜像：
   ```bash
   docker build -t rsde:test .
   ```

2. 配置文件已准备：`../../manifest/dev/remote_ocr.toml`

## 快速启动

```bash
# 在项目根目录
cd k8s/docker-compose

# 启动服务
docker-compose up -d

# 查看日志
docker-compose logs -f apiserver

# 停止服务
docker-compose down
```

## 服务说明

### apiserver

- **端口**: 3000
- **健康检查**: `http://localhost:3000/ocr/health`
- **API 端点**: 
  - `POST http://localhost:3000/ocr/single_pic` - 远程 OCR
  - `POST http://localhost:3000/ocr/single_pic_local` - 本地 OCR
  - `GET http://localhost:3000/ocr/health` - 健康检查
- **前端**: `http://localhost:3000/` - React Web UI

### 挂载卷

| 主机路径 | 容器路径 | 说明 | 只读 |
|---------|---------|------|-----|
| `../../manifest/dev/remote_ocr.toml` | `/app/config/remote_ocr.toml` | API 配置文件 | ✅ |
| `../../log` | `/app/log` | 日志目录 | ❌ |
| `../../manifest/dev/train_data` | `/app/train_data` | OCR 训练数据 | ✅ |

## 配置定制

### 修改端口

编辑 `docker-compose.yaml`：

```yaml
ports:
  - "8080:3000"  # 主机端口:容器端口
```

### 使用自定义配置

1. 创建新配置文件 `custom.toml`
2. 修改 `docker-compose.yaml` 的 volumes 和环境变量：

```yaml
environment:
  - API_CONFIG=/app/config/custom.toml
volumes:
  - ./custom.toml:/app/config/custom.toml:ro
```

### 调整日志级别

编辑配置文件中的 `log_level`：

```toml
[apiserver]
log_level = "debug"  # trace, debug, info, warn, error
```

## 测试 API

```bash
# 健康检查
curl http://localhost:3000/ocr/health

# OCR 识别（需要准备图片）
curl -X POST http://localhost:3000/ocr/single_pic \
  -H "Content-Type: application/json" \
  -d '{
    "image_path": "/path/to/image.png",
    "include_position": true
  }'
```

## 访问 Web UI

浏览器打开 http://localhost:3000

- **首页**: 工具概览
- **Rsync**: 文件同步工具
- **RC**: 远程命令工具
- **OCR**: 图片文字识别

## 故障排查

### 服务无法启动

```bash
# 查看容器状态
docker-compose ps

# 查看详细日志
docker-compose logs apiserver

# 检查配置文件是否存在
ls -la ../../manifest/dev/remote_ocr.toml
```

### 健康检查失败

可能需要安装 curl（已在 Dockerfile 中包含 ca-certificates，如需 curl 可修改 Dockerfile）：

```dockerfile
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

或使用 wget 替代：

```yaml
healthcheck:
  test: ["CMD", "wget", "--quiet", "--tries=1", "--spider", "http://localhost:3000/ocr/health"]
```

### 前端页面 404

确保镜像构建时包含了前端资源：

```bash
# 重新构建镜像
docker build -t rsde:test .

# 重启容器
docker-compose down
docker-compose up -d
```

## 生产部署建议

1. **使用版本标签**：
   ```yaml
   image: rsde:v1.0.0
   ```

2. **持久化日志**：
   ```yaml
   volumes:
     - ./logs:/app/log
   ```

3. **环境变量管理**：
   ```bash
   # 使用 .env 文件
   echo "API_CONFIG=/app/config/remote_ocr.toml" > .env
   ```

4. **反向代理**（Nginx/Traefik）：
   - 添加 HTTPS
   - 负载均衡
   - 访问控制

5. **监控告警**：
   - 集成 Prometheus/Grafana
   - 日志聚合（ELK/Loki）

## 多服务编排示例

如果需要运行多个服务：

```yaml
services:
  apiserver:
    # ... 现有配置 ...

  rsync-service:
    image: rsde:test
    command: ["rsync"]
    volumes:
      - ./rsync-config.toml:/app/config.toml:ro
    restart: unless-stopped

  rc-service:
    image: rsde:test
    command: ["rc"]
    restart: unless-stopped
```
