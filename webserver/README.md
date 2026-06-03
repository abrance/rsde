# WebServer Frontend

> 注意：此目录仅包含前端代码。前端服务已集成到 apiserver 中。

## 架构说明

WebServer 的前端 UI 已合并到 `apiserver` 中，现在 `apiserver` 同时提供：
- API 服务（`/api/*` 路径）
- 前端静态文件服务（根路径）

## 开发

### 前端开发

前端独立开发模式使用 Vite dev server：

```bash
cd frontend

# 安装依赖
npm install

# 启动开发服务器（默认端口 5173）
npm run dev

# 运行 lint
npm run lint

# 运行测试
npm run test

# 构建生产版本
npm run build

# 预览生产构建
npm run preview
```

说明：

- Vite dev server 默认监听 `http://localhost:5173`。
- 开发模式下，前端发往 `/api/*` 的请求会代理到 `http://localhost:3000`。
- 这里的 `http://localhost:3000` 指向本地 `apiserver`，用于承接后端 API。

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

完整服务模式说明：

- 这里描述的是**集成运行模式**，不是 Vite dev server 模式。
- 在该模式下，frontend 构建产物位于 `webserver/frontend/dist/`，并由 `apiserver` 在根路径提供静态文件服务。
- 此时用户访问地址为 `http://localhost:3000`。

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
集成运行时，请求由 `apiserver` 直接处理，不再经过 Vite dev server。
