# RSDE WebServer Frontend

基于 React + TypeScript + Vite 构建的 RSDE 工具集 Web UI。

## 技术栈

- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Vite** - 构建工具
- **React Router** - 路由管理
- **Axios** - HTTP 客户端

## 开发

```bash
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

- `npm run dev` 启动的是 Vite dev server，默认地址为 `http://localhost:5173`。
- `npm run build` 生成的静态文件位于 `dist/`。
- `npm run preview` 用于本地预览构建结果，但它不等同于由 `apiserver` 托管的集成运行模式。

## 运行模式

前端相关文档里需要区分两种模式：

### 1. Vite 开发模式

- 使用 `npm run dev`
- 前端页面访问地址默认是 `http://localhost:5173`
- `/api/*` 请求由 Vite 代理到本地 `apiserver`

### 2. apiserver 集成运行模式

- 使用 `npm run build` 先构建 frontend
- 再从仓库根目录启动 `cargo run -p apiserver --release`
- 最终由 `apiserver` 提供前端静态资源和后端 API
- 用户访问地址是 `http://localhost:3000`

## 项目结构

```
src/
├── components/       # 共享组件
│   └── Layout.tsx   # 页面布局
├── pages/           # 页面组件
│   ├── HomePage.tsx # 首页
│   ├── RsyncPage.tsx # Rsync 工具页
│   ├── RcPage.tsx   # RC 配置管理页
│   └── OcrPage.tsx  # OCR 识别页
├── App.tsx          # 应用入口
└── main.tsx         # React 挂载点
```

## 功能模块

### 首页
- 工具集介绍
- 核心特性展示
- 快速导航

### Rsync 数据同步
- 概览和功能介绍
- 配置管理（开发中）
- 实时监控（开发中）

### RC 远程配置
- 配置管理功能
- 多环境支持
- 配置历史（开发中）

### OCR 图片识别
- 图片文字识别
- 坐标信息提取
- 识别历史（开发中）

## API 代理

开发环境下，所有 `/api` 开头的请求会被代理到 `http://localhost:3000`。

配置位于 `vite.config.ts`。

## 部署

构建后的静态文件在 `dist/` 目录，可以由 `apiserver` 直接提供服务。

```bash
npm run build
# dist/ 目录包含所有静态资源
```
