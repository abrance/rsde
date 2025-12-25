# WebServer Frontend

> 注意：此目录仅包含前端代码。前端服务已集成到 apiserver 中。

## 架构说明

WebServer 的前端 UI 已合并到 `apiserver` 中，现在 `apiserver` 同时提供：
- API 服务（`/api/*` 路径）
- 前端静态文件服务（根路径）

## 开发

### 前端开发

```bash
cd frontend

# 安装依赖
npm install

# 启动开发服务器（默认端口 5173）
npm run dev

# 构建生产版本
npm run build
```

### 启动完整服务

```bash
# 1. 构建前端
cd webserver/frontend
npm install
npm run build
cd ../..

# 2. 启动 apiserver（会自动提供前端服务）
cargo run -p apiserver --release
```

访问 http://localhost:3000 即可看到前端界面。

## 目录结构

```
webserver/
├── frontend/           # React 前端项目
│   ├── src/           # 前端源码
│   ├── dist/          # 构建产物（由 apiserver 提供服务）
│   ├── package.json
│   └── vite.config.ts
└── README.md          # 本文件
```

## API 路由

所有 API 请求都通过 `/api` 前缀：

- `POST /api/ocr/single_pic` - OCR 识别
- `GET /api/ocr/health` - 健康检查

前端开发时，Vite 会自动代理 `/api/*` 请求到 `http://localhost:3000`。
