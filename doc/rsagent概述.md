# rsagent 概述

## rsagent 是什么

`rsagent` 是部署在被管理节点上的轻量代理程序，负责把节点接入到 `NodeManage` 体系中，并持续承担节点侧的注册、心跳指标上报、能力上报、指令执行与状态回传职责。

它的核心作用是把原本依赖人工 SSH、脚本分发、临时命令执行的节点管理方式，收敛成一个长期在线、可观测、可审计、可扩展的节点接入与控制通道。

一句话定位：

> `rsagent` 是 NodeManage 的节点侧执行与通信代理，是节点被平台稳定纳管的基础设施。

## rsagent 解决什么问题

### 1. 节点接入成本高

没有标准代理时，新增节点通常需要人工 SSH 到目标机器，手动执行安装命令、配置运行环境、分发脚本，并在平台侧登记节点信息。

这种方式容易出现步骤不一致、信息登记不完整、安装过程不可追踪等问题。`rsagent` 将“安装、注册、纳管”变成标准化流程，让节点接入可以被平台统一编排和追踪。

### 2. 节点状态不透明

如果节点侧没有常驻代理，平台只能记录节点的静态信息，很难判断节点当前是否在线、服务是否正常、网络是否可达、节点具备哪些能力。

`rsagent` 通过持续上报心跳指标、基础状态和能力信息，让平台可以判断节点是否在线、是否异常、具备哪些能力。心跳指标不直接由 NodeManage 自行计算，也不应该是一条隐含写入链路；节点 heartbeat 链路需要先由 datalink-engine 创建和纳管，再写入 VictoriaMetrics，并由独立的 query-engine 组件统一查询 heartbeat 数据，再由 `nodemanage` 计算节点状态。

### 3. 远程操作缺少稳定通道

SSH 适合完成首次安装和少量临时操作，但不适合作为长期节点管理通道。长期依赖 SSH 会带来凭据管理复杂、操作不可审计、执行结果难追踪等问题。

`rsagent` 提供 NodeManage 到节点的稳定通信载体。后续可以基于它扩展任务下发、日志回传、脚本执行、文件分发、配置同步等能力。

### 4. 节点管理缺乏产品化基础

如果没有节点侧代理，NodeManage 很容易停留在“节点信息 CRUD 系统”：可以新增、查询、删除节点记录，但无法真正感知和控制节点。

有了 `rsagent`，NodeManage 才具备从“记录节点”升级为“管理节点”的基础，能够形成节点接入、状态观测、远程执行、结果追踪的完整闭环。

## 与 NodeManage 的关系

NodeManage 是平台侧的节点管理服务，负责节点信息管理、安装流程编排和 API 暴露。

`rsagent` 是节点侧的执行与通信代理，负责在目标节点上完成注册、心跳指标上报、状态上报和后续任务执行。

二者的关系可以理解为：

- NodeManage 负责“管理视角”和“控制平面”。
- `rsagent` 负责“节点视角”和“执行平面”。
- `job-manage`（也可简称 `jm`）负责“作业平台”和“任务编排平面”，真正把脚本或命令任务下发到 NodeManage 纳管的节点。
- `rsagent` 作为节点侧代理，负责接收 `job-manage` 下发的脚本或命令任务，并在节点侧执行后回传结果。
- datalink-engine 负责“数据链路登记、result_table 映射和存储位置管理”。
- VictoriaMetrics 负责“节点指标存储”。
- query-engine 负责“heartbeat 数据查询”。

## 第一阶段目标

`rsagent` 第一阶段不追求复杂的远程控制能力，但除了打通节点纳管闭环之外，还需要明确 `job-manage`（`jm`）与 `rsagent` 的通信方式，并支持由 `jm` 向纳管节点下发脚本或命令执行任务：

1. NodeManage 通过 SSH 在线安装 `rsagent`。
2. `rsagent` 启动后向 NodeManage 注册节点。
3. NodeManage 记录节点注册信息和唯一标识，不直接根据数据库字段判定节点在线。
4. NodeManage 首次启动时请求 datalink-engine 声明式创建节点 heartbeat 数据链路，获得链路唯一 ID 和对应 `result_table`。
5. NodeManage 安装 `rsagent` 时下发配置文件，配置中包含 heartbeat 的 `data_link_id`。
6. `rsagent` 周期性按该数据链路定义向 VictoriaMetrics 上报本节点 heartbeat 指标，指标维度包含节点唯一标识信息。
7. `rsagent` 定期通过 datalink-engine 查询 `data_link_id` 对应的链路信息，例如每 5 分钟同步一次配置。
8. query-engine 通过 datalink-engine 按 `data_link_id` 获取 heartbeat 链路的 `result_table` 和存储映射，再查询 VictoriaMetrics 返回 heartbeat 数据。
9. NodeManage 通过 query-engine 获取 heartbeat 数据并计算节点状态，再在节点列表或详情页中展示。
10. `job-manage` 与 `rsagent` 之间需要确定稳定通信方式，用于向节点下发脚本或命令任务。
11. `rsagent` 需要支持接收 `job-manage` 下发的脚本或命令，并在节点侧执行后回传完整执行结果。

在这个阶段里，真正面向纳管节点下发脚本和 task 的平台应是 `job-manage`（`jm`）：

- `NodeManage` 负责节点纳管和节点选择范围；
- `job-manage` 负责创建作业、选择节点，并通过与 `rsagent` 的通信通道下发脚本或命令任务；
- `rsagent` 负责在节点侧接收、执行并回传结果；
- 因此，`jm ↔ rsagent` 的通信协议和脚本下发执行主链路，属于第一阶段就需要明确的设计内容。

这个阶段的重点是让节点能够被稳定接入、识别和观测，并明确 NodeManage、job-manage、rsagent、datalink-engine、VictoriaMetrics、query-engine 之间的职责边界，为后续协议设计、任务下发和远程执行能力打基础。
