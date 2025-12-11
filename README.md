## 概述

rsde(xy) 项目是一个基于 rust 和前端语言开发的运行于 k8s 平台的 saas 应用，为一些运维人员提供便捷的工具服务。

rc 是远程探测各个组件状态的命令行工具。

rsync 是一个基于规则引擎的文件（数据）同步工具。支持多种数据源和多种数据目标，支持复杂的规则引擎配置。

将驱动 AI , 万物互联的产品, 数据源对接到大模型, 实现 AI 助手功能. 

## 使用场景

- 对接企微数据, 你可以告诉 xy 你可能关注的信息, 如 xx 项目有进展了请通知我. xy 将监控企微消息, 并在有相关消息时通知你.
- 对接会议,将整理会议纪要, 生成会议行动项.
- 对接 k8s 集群,告诉你目前集群的状态, 以及异常时的处理建议.
- 对接工单系统, 帮助你分析工单内容, 提供处理建议. 

## 目录结构

- rc: 远程探测工具
- rsync: 基于规则引擎的文件（数据）同步工具
- docs: 项目相关文档
- common: 后端公共库
- apiserver: 后端服务
- pic_recog: 图像识别模块

- bin: 各种脚本和可执行文件
- log: 日志文件

## 特性

- frontend web ui(todo)
- backend apiserver
- log
- github workflow
- health check url
- configuration file support(toml)
- metrics server(prometheus )  (todo)
- helm chart
- helm files (todo)
- MCP support (todo)
