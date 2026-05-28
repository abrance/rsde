# nodemanage 概述

## nodemanage 是什么

`nodemanage` 是节点管理体系中的平台侧核心服务，负责把一批原本分散、依赖人工维护的机器节点，纳入统一的管理平面。

它不直接承担节点侧执行，也不直接解释底层指标存储，而是负责：

- 节点接入与纳管流程编排；
- 节点身份、基础信息和安装记录管理；
- 与 `rsagent`、`datalink-engine`、`query-engine`、`job-manage` 的协作调度；
- 对前端和其他平台服务暴露统一的节点管理 API。

一句话定位：

> `nodemanage` 是节点管理体系的控制平面，负责节点纳管、安装编排、节点元信息管理和跨组件协同。

## nodemanage 解决什么问题

### 1. 节点管理不能停留在手工登记

如果没有独立的节点管理服务，平台通常只能维护一份“机器清单”，例如 IP、主机名、环境、负责人。这种模式只能解决“记录节点”的问题，无法解决“让节点被稳定接入平台并持续可管理”的问题。

`nodemanage` 负责把节点从静态记录升级为可纳管对象：节点可以被安装 agent、拥有稳定身份、被周期性计算运行状态、被执行任务，并被纳入统一审计和运维流程。

### 2. 节点接入流程需要统一编排

首次纳管节点往往涉及多个步骤，例如录入节点、下发安装参数、通过 SSH 安装 `rsagent`、生成配置、回写节点唯一标识、登记 heartbeat 链路。

这些动作如果散落在脚本、人工操作或多个服务里，流程会变得不可追踪、不可回放，也难以排障。

`nodemanage` 将节点接入收敛成标准流程，让“安装、注册、绑定链路、状态可见”成为一条明确的控制面闭环。

### 3. 节点在线状态不应该由管理库字段硬编码判断

节点是否在线，本质上是运行态问题，不是管理表里一个静态布尔字段就能正确表达的。

`nodemanage` 不直接根据自身数据库字段判断在线状态，而是周期性通过 `query-engine` 查询由 `rsagent` 上报、并由 `datalink-engine` 纳管的 heartbeat 链路时序数据，再按 `node_id` 维度和自身规则计算节点状态。这样节点状态才具备可解释性和可追溯性。

### 4. 平台需要统一的节点控制入口

后续无论是任务执行、批量脚本分发、配置同步、巡检，还是日志采集、资源观测，都需要围绕一个统一的节点主体展开。

`nodemanage` 提供这个主体模型和控制入口。没有它，`job-manage`、`query-engine`、`rsagent` 会变成彼此松散耦合的组件；有了它，整套系统才有稳定的控制平面和领域边界。

## 核心职责

当前阶段建议将 `nodemanage` 的职责边界明确为以下几类。

### 1. 节点元信息管理

负责维护节点的管理态信息，例如：

- 节点唯一标识；
- 节点名称、IP、环境、所属集群；
- 节点来源和纳管方式；
- 节点安装状态、注册状态、最近安装任务记录；
- 节点标签和分组信息。

这些信息用于“识别节点、选择节点、组织节点”，但不直接等价于运行态健康状态。

### 2. 安装与纳管编排

负责发起并跟踪节点纳管流程，例如：

1. 用户在平台中录入待纳管节点；
2. `nodemanage` 生成安装计划；
3. `nodemanage` 调用 SSH 或安装器在目标节点安装 `rsagent`；
4. 安装时下发包含 `data_link_id` 的配置；
5. `rsagent` 启动后向平台注册；
6. `nodemanage` 记录注册结果并进入“已纳管”状态。

### 3. 数据链路依赖管理

`nodemanage` 自身不管理 heartbeat 数据存储，但需要管理自己依赖的链路引用。

第一阶段中，`nodemanage` 需要：

- 在启动或初始化阶段向 `datalink-engine` 申请/对齐 heartbeat 链路；
- 保存该链路的 `data_link_id`；
- 在安装 `rsagent` 时把 `data_link_id` 下发到节点配置中；
- 在周期性状态刷新时，把该 `data_link_id` 提供给 `query-engine` 查询 heartbeat 数据。

### 4. 状态聚合与对外 API

`nodemanage` 对外暴露的平台接口应该是“节点管理语义”，而不是底层指标语义。

例如：

- 创建节点；
- 发起节点纳管；
- 查询节点详情；
- 查询节点安装状态；
- 查询节点当前在线状态；
- 按标签或分组筛选节点。

其中在线状态、最近心跳时间、基础健康状态等运行态数据，应由 `nodemanage` 基于 `query-engine` 提供的 heartbeat 查询结果自行计算和聚合后，再以节点管理语义返回给上层。

## 与其他组件的关系

- `nodemanage`：控制平面，负责节点纳管流程、管理态信息和平台 API。
- `rsagent`：节点侧代理，负责注册、heartbeat 上报、配置同步和后续节点执行能力。
- `datalink-engine`：负责 heartbeat 等链路的元数据登记、`result_table` 映射和链路治理。
- `query-engine`：负责从链路元数据出发查询 heartbeat 底层数据，并返回统一查询结果。
- `job-manage`：消费 `nodemanage` 提供的节点清单和节点状态，并在执行前基于 `nodemanage` 的状态结果判断节点是否可执行任务。

可以把这套关系理解为：

- `nodemanage` 管“节点对象”和“控制面流程”；
- `rsagent` 管“节点侧接入”和“执行面能力”；
- `datalink-engine` 管“数据链路定义”；
- `query-engine` 管“heartbeat 数据查询”；
- `job-manage` 管“基于节点的远程任务执行”。

## 核心对象建议

为了让后续详细设计更容易展开，建议先把 `nodemanage` 关注的核心对象收敛为以下几类。

### 1. Node

节点主对象，表示一个被平台识别和纳管的节点。

建议至少包含：

- `node_id`：平台内唯一标识；
- `node_name`：节点名称；
- `host` / `ip`：访问地址；
- `environment`：环境，例如 dev/test/prod；
- `labels`：业务或运维标签；
- `register_status`：注册状态；
- `install_status`：安装状态；
- `agent_id`：已注册 agent 标识；
- `created_at` / `updated_at`。

### 2. NodeInstallTask

用于记录一次节点纳管安装流程。

建议包含：

- 安装任务 ID；
- 目标节点 ID；
- 执行方式（SSH / installer）；
- 发起人；
- 当前阶段；
- 执行结果；
- 错误信息；
- 开始/结束时间。

这个对象解决的是“节点为什么没有纳管成功”以及“某次安装过程卡在哪一步”的问题。

### 3. NodeAgentBinding

用于表达节点对象和 `rsagent` 实例之间的绑定关系。

建议至少记录：

- `node_id`；
- `agent_id`；
- 首次注册时间；
- 最近一次注册/握手时间；
- 当前绑定状态。

### 4. NodeDataLinkRef

用于维护 `nodemanage` 依赖的数据链路引用，而不是把链路信息硬编码进业务逻辑。

第一阶段至少包含 heartbeat 链路：

- `domain = nodemanage`
- `link_purpose = node_heartbeat`
- `data_link_id`
- `owner_service = nodemanage`

## 第一阶段目标

`nodemanage` 第一阶段不追求复杂调度，而是优先打通“节点被平台稳定纳管并可见状态”的最小闭环：

1. 支持录入待纳管节点基础信息。
2. 支持对节点发起安装流程，通过 SSH 安装 `rsagent`。
3. 启动时向 `datalink-engine` 对齐 heartbeat 链路，并保存 `data_link_id`。
4. 安装 `rsagent` 时下发包含 heartbeat `data_link_id` 的配置。
5. 支持 `rsagent` 注册回调或注册上报，完成节点与 agent 的绑定。
6. 支持周期性通过 `query-engine` 查询 heartbeat 数据，并由 `nodemanage` 计算节点在线状态和最近心跳时间。
7. 在节点列表和节点详情页中展示管理态信息和运行态状态。

第一阶段完成后，平台将不再只是“维护节点列表”，而是具备完整的节点纳管主链路。

## 第一阶段闭环时序

建议把第一阶段主链路理解为如下时序：

1. `nodemanage` 启动。
2. `nodemanage` 调用 `datalink-engine` `ApplyDataLink` 创建或对齐 heartbeat 链路。
3. `nodemanage` 保存 `node_heartbeat` 对应的 `data_link_id`。
4. 用户创建待纳管节点并发起安装。
5. `nodemanage` 通过 SSH 在目标节点安装 `rsagent`，并写入配置文件。
6. 配置文件中包含节点身份信息、平台地址、heartbeat `data_link_id` 等配置。
7. `rsagent` 启动后向 `nodemanage` 注册。
8. `rsagent` 周期性按 `data_link_id` 对应链路定义上报 heartbeat 到 VictoriaMetrics。
9. `query-engine` 通过 `data_link_id` 获取链路和 `result_table`，查询 heartbeat 数据。
10. `nodemanage` 周期性调用 `query-engine`，计算节点在线状态和最近心跳时间，并向前端返回结果。

这个时序的关键价值在于：

- 节点身份归 `nodemanage` 管；
- heartbeat 链路归 `datalink-engine` 管；
- 在线状态解释归 `nodemanage` 管；
- 节点侧动作归 `rsagent` 管。

这样边界清晰，后续每个组件都可以独立演进。

## 当前明确不做的事情

为了避免第一阶段目标失焦，以下能力建议明确不纳入 `nodemanage` 第一阶段：

- 不在 `nodemanage` 内直接实现底层指标查询逻辑；
- 不在 `nodemanage` 内直接实现底层 heartbeat 查询细节；
- 不在 `nodemanage` 内承载复杂批量任务调度；
- 不把 heartbeat 存储位置、指标名、PromQL 规则硬编码进节点管理逻辑；
- 不把 `rsagent` 的执行协议细节直接耦合进节点主对象模型。

这些边界如果不提前说明，后续很容易让 `nodemanage` 演变成“什么都管一点”的耦合中心。

## 后续详细设计建议

在这篇概述之后，建议继续补四类详细设计文档：

1. **nodemanage API 契约**：节点创建、节点安装、节点详情、节点状态查询接口。
2. **nodemanage 数据模型设计**：Node、NodeInstallTask、NodeAgentBinding、NodeDataLinkRef 的字段和状态机。
3. **rsagent 注册与配置协议**：注册请求、配置拉取、配置版本、错误处理。
4. **安装与纳管时序设计**：从 SSH 安装到状态可见的完整交互时序和失败回滚策略。

这样整套节点管理体系就会从“组件概述”升级为“可执行的详细设计”。
