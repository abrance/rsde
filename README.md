## 概述

rsde(xy) 项目是一个基于 Rust 开发的运行于 Kubernetes 平台的工具集，为运维人员提供便捷的服务。

**核心工具：**
- **rsync** - 基于规则引擎的文件（数据）同步工具，支持多种数据源和目标
- **rc** - 远程配置管理工具
- **pic_recog** - 图片文字识别服务（OCR）
- **apiserver** - 统一的 API 网关，提供 Web UI 和 API 服务

将驱动 AI，万物互联的产品，数据源对接到大模型，实现 AI 助手功能。

## 快速开始

### 使用启动脚本（推荐）

```bash
# 一键启动（自动构建前端和后端）
./start.sh
```

访问 http://localhost:3000 查看 Web UI

### 手动启动

```bash
# 1. 构建前端
cd webserver/frontend
npm install
npm run build
cd ../..

# 2. 启动服务
cargo run -p apiserver --release
```

## 使用场景

- 对接企微数据，监控企微消息并在有相关消息时通知
- 对接会议，整理会议纪要，生成会议行动项
- 对接 K8s 集群，告诉你目前集群的状态以及异常处理建议
- 对接工单系统，帮助分析工单内容，提供处理建议
- OCR 图片文字识别

## 项目结构

```
rsde/
├── apiserver/          # API 网关（提供 Web UI 和 API）
├── rsync/             # 文件同步工具
├── rc/                # 远程配置管理
├── pic_recog/         # OCR 识别模块
├── webserver/
│   └── frontend/      # React 前端（集成到 apiserver）
├── common/            # 公共库
│   ├── config/       # 统一配置管理
│   ├── core/         # 核心功能
│   └── util/         # 工具函数
├── bin/              # 编译产物
├── manifest/         # 配置文件
└── start.sh          # 快速启动脚本
```

## 核心特性

- ✅ 统一的 Web UI（React + TypeScript + Vite）
- ✅ RESTful API 服务
- ✅ 配置文件支持（TOML）
- ✅ 日志系统
- ✅ GitHub Actions CI/CD
- ✅ Docker 容器化部署
- ✅ Helm Chart 支持
- 🚧 Prometheus Metrics（开发中）
- 🚧 MCP 协议支持（开发中）
