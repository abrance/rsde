# nodemanage 前后端协议设计

## 文档目标

本文用于冻结 `nodemanage` 第一阶段面向前端的稳定协议。

它解决的不是最终形态的声明式控制面协议，而是当前阶段最急需的联调问题：

- 前端如何稳定获取节点列表与节点详情；
- 前端如何追踪节点安装过程；
- 前端如何查询绑定关系与批量状态；
- 后端如何在保留现有动作接口的同时，提供稳定读模型；
- 前后端如何基于统一错误模型展示失败原因与重试语义。

本文的重点是 **先冻结稳定查询面和任务面**，而不是一步切换到完整的 `NodeResource(metadata/spec/status)` + reconcile engine 目标架构。

## 设计背景

结合当前仓库已有文档与代码现状：

- `doc/nodemanage概述.md` 与 `doc/nodemanage详细设计.md` 已经明确了 `Node`、`NodeInstallTask`、`NodeAgentBinding`、`NodeDataLinkRef` 等领域方向；
- `apiserver/src/nodemanage.rs` 当前仍以动作型接口为主，例如 `/install`、`/agent/sync`、`/node/:id/status/refresh`；
- `nodemanage/src/protocol.rs` 已经有 `AgentSyncRequest` / `AgentSyncResponse` 等 agent 侧协议雏形；
- 当前最直接阻塞 NodeManage 前后端联调的，不是完整 reconcile engine 缺失，而是 **缺少稳定的前端可消费协议面**。

因此，第一阶段协议设计应优先回答：

1. 前端到底读什么对象；
2. 哪些动作接口继续保留；
3. 安装任务如何变成可查询对象；
4. 批量状态如何表达；
5. 错误如何稳定展示。

## 设计原则

### 1. 查询面优先冻结

第一阶段优先冻结前端稳定消费的查询接口、读模型和错误模型。

原因是：

- 前端页面首先需要稳定拿到可展示数据；
- 安装、绑定、状态刷新等过程需要可观察；
- 如果先冻结查询面，后续后端内部实现仍可渐进演进。

### 2. 动作面保留过渡，不立即废弃

当前后端已经存在动作型接口和命令式流程。

第一阶段不要求立即废弃：

- 安装接口继续保留；
- rebind 接口继续保留；
- refresh 接口继续保留；
- agent sync/register 接口继续保留。

但这些接口的定位应收敛为：

- 过渡接口；
- 内部流程入口；
- 非前端主依赖协议。

前端主依赖应迁移到稳定查询面。

### 3. DTO 命名向目标态靠拢，但不一步做全

第一阶段不强制完整暴露 `metadata/spec/status` 三段式资源树，
但字段命名与职责边界应尽量向目标态靠拢，例如：

- `lifecycle_state`
- `install_phase`
- `binding_state`
- `online_status`

这样后续升级到声明式资源时，前端迁移成本更低。

### 4. 安装过程必须提升为任务对象

安装过程不能只通过一次性 `/install` 响应返回。

第一阶段必须把安装过程提升为可查询任务对象，以满足：

- 进度可追踪；
- 失败可排障；
- 结果可审计；
- 前端可重试；
- 后端后续可演进状态机。

### 5. 多组件交互先对齐协议，再推进实现

凡是涉及 `nodemanage` 与 `rsagent`、`datalink-engine`、`query-engine` 或前端的跨组件交互，必须先有明确协议面。

这条原则适用于：

- heartbeat datalink 引用下发；
- agent 注册与绑定；
- 批量状态聚合；
- 安装任务状态对外表达。

## 协议分层

第一阶段建议把 NodeManage 前后端协议分为四层。

### 1. 稳定查询面

这是前端应优先依赖的读接口层，用于页面展示和状态轮询。

职责包括：

- 节点列表查询；
- 节点详情查询；
- 批量状态查询；
- 绑定关系查询；
- 安装任务查询。

### 2. 稳定任务面

这是安装过程的可观察模型层。

职责包括：

- 返回当前安装任务状态；
- 返回最近一次安装任务；
- 暴露任务错误与是否可重试；
- 为后续安装状态机提供稳定外部表示。

### 3. 过渡动作面

这是第一阶段仍保留的动作型接口层。

职责包括：

- 发起安装；
- 手工 rebind；
- 手工刷新状态；
- agent sync/register。

这些接口可以继续存在，但不再作为前端页面的主读取接口。

### 4. 稳定错误面

这是前后端共享的错误表达层。

职责包括：

- 冻结错误码；
- 区分可重试与不可重试错误；
- 向前端返回可展示、可定位、可归类的失败信息。

## 第一阶段绑定 / rebind 协议补充

在第一阶段节点列表/详情/安装任务查询面冻结之后，前端还需要一个明确的绑定修正动作，来处理：

- 节点当前绑定了错误的 agent；
- 节点重装后需要把控制关系显式切换到目标 agent；
- `/agent/sync` 已经能暴露冲突，但前端缺少稳定的修正动作协议。

因此第一阶段补充冻结 **forced rebind** 协议。

### 1. 动作接口

- `POST /api/nm/v1/nodes/:node_id/rebind`

请求模型：

- `target_agent_id: string`
- `reason: string | null`

约束：

- `target_agent_id` 必填；
- 空字符串或纯空白字符串请求必须拒绝；
- `reason` 仅用于审计/操作说明，不改变查询面结构。

### 2. 响应模型

`RebindNodeResponse`

- `accepted: boolean`
- `node_id: string`
- `target_agent_id: string`
- `binding_state: string`
- `previous_agent_id: string | null`

说明：

- 这是动作结果，不是完整详情投影；
- 前端在动作成功后，应回到稳定查询接口重新拉取详情或绑定信息；
- 第一阶段响应保持轻量，不在动作响应中重复完整节点聚合视图。

### 3. 状态迁移语义

第一阶段 forced rebind 采用以下语义：

1. 校验节点存在；
2. 校验目标 agent 在仓库中已有绑定记录；
3. 若 `target_agent_id` 为空白，则返回参数错误；
4. 若目标 agent 当前已经 `bound` 到其它节点，则拒绝隐式抢占；
5. 若当前节点已有不同的 `bound` agent，则旧绑定转为 `stale`；
6. 目标 agent 绑定切换为当前节点的 `bound`；
7. 前端通过稳定查询接口读取动作后的最终状态。

这里明确要求：

- `stale` 用于表示“曾经有效但不再是当前权威绑定”的旧关系；
- 第一阶段不删除被替换的旧绑定记录；
- 第一阶段不把成功 rebind 的稳定结果表示为 `conflict`。

### 4. 前端 readback 约束

前端不应把 `POST /api/nm/v1/nodes/:node_id/rebind` 的返回当作长期展示源，而应在动作成功后重新读取：

- `GET /api/nm/v1/nodes/:node_id`
- `GET /api/nm/v1/nodes/:node_id/binding`
- 或 `GET /api/nm/v1/nodes/status:batch`

这样可以继续保持第一阶段协议约束：

- 动作接口负责触发状态变化；
- 稳定查询接口负责页面展示。

## 第一阶段主读模型

### 1. NodeSummary

`NodeSummary` 用于节点列表页、批量筛选页和表格场景。

建议字段：

- `node_id: string`
- `node_name: string`
- `environment: string`
- `labels: string[]`
- `lifecycle_state: string`
- `install_phase: string`
- `binding_state: string`
- `online_status: string`
- `last_heartbeat_at: string | null`
- `updated_at: string`

字段语义建议：

- `lifecycle_state` 表达节点主体生命周期，例如 `draft` / `onboarding` / `managed`；
- `install_phase` 表达安装流程状态，例如 `not_started` / `installing` / `waiting_register` / `installed` / `install_failed`；
- `binding_state` 表达节点与 agent 的绑定关系，例如 `bound` / `stale` / `unbound`；
- `online_status` 表达运行态聚合结果，例如 `online` / `offline` / `unknown`。

这里明确要求：

- 列表页不再依赖当前扁平 `Node.status` 直接表达所有含义；
- 不同状态轴必须分离；
- 前端不自行推断安装/绑定/在线状态的组合语义。

### 2. NodeDetail

`NodeDetail` 用于节点详情页，建议组织为聚合对象：

```json
{
  "node": {},
  "binding": {},
  "status": {},
  "latest_install_task": {},
  "heartbeat_ref": {}
}
```

建议结构：

- `node`
  - 基础管理态信息：`node_id`、`node_name`、`access_host`、`environment`、`labels`、`created_at`、`updated_at`
- `binding`
  - 当前 agent 绑定关系：`agent_id`、`binding_state`、`first_registered_at`、`last_handshake_at`
- `status`
  - 聚合状态：`lifecycle_state`、`install_phase`、`register_status`、`online_status`、`last_heartbeat_at`、`status_reason`
- `latest_install_task`
  - 最近一条安装任务摘要
- `heartbeat_ref`
  - 当前 heartbeat datalink 引用摘要：`data_link_id`、`link_purpose`、`owner_service`、`result_table_name`

这一定义的重点是：

- 详情页读取的是真正的聚合对象，而不是多个接口在前端拼装；
- 后端负责把管理态、任务态、绑定态、运行态组合为一份稳定视图；
- `heartbeat_ref` 只暴露引用摘要，不泄露 datalink-engine 内部全部真相模型。

### 3. NodeStatusBatchItem

用于列表页批量状态刷新场景，建议字段：

- `node_id`
- `install_phase`
- `binding_state`
- `online_status`
- `last_heartbeat_at`
- `updated_at`
- `status_reason`

这类对象的目标是：

- 让前端以较低成本轮询核心状态；
- 不必每次重新拉整份详情；
- 让后端后续能对状态投影做专门优化。

## 第一阶段安装任务模型

### 1. NodeInstallTaskView

第一阶段不再直接把领域对象 `NodeInstallTask` 作为前后端共享对象，而是冻结面向前端的稳定读模型 `NodeInstallTaskView`。

建议字段：

- `install_task_id: string`
- `node_id: string`
- `task_state: string`
- `current_step: string | null`
- `error_code: string | null`
- `error_message: string | null`
- `started_at: string | null`
- `finished_at: string | null`
- `retryable: boolean`
- `request_summary: InstallRequestSummary | null`

其中：

- `NodeInstallTask` 继续保留为后端领域/持久化对象；
- `NodeInstallTaskView` 作为稳定查询协议；
- 前端不直接依赖领域对象内部字段。

### 2. InstallRequestSummary

`InstallRequestSummary` 用于向前端返回脱敏后的安装请求摘要，建议字段：

- `host: string | null`
- `ssh_port: number | null`
- `username: string | null`
- `rsagent_package_url: string | null`
- `install_root: string | null`
- `labels: string[]`
- `plugin_names: string[]`

明确约束：

- `password` 不进入查询面；
- `private_key` 不进入查询面；
- 后续任何 credential / token / secret 类字段都不进入查询面。

### 3. task_state 枚举建议

- `pending`
- `running`
- `waiting_register`
- `succeeded`
- `failed`
- `cancelled`

### 4. current_step 建议值

第一阶段不必冻结得过细，但建议至少可表达：

- `prepare_install`
- `resolve_artifacts`
- `write_runtime_config`
- `upload_package`
- `run_install_script`
- `start_agent`
- `wait_register`

这样做的价值在于：

- 前端失败提示可以更具体；
- 后端排障路径更短；
- 后续状态机实现时不需要重做外部字段。

### 5. retryable 语义

`retryable` 表示当前任务失败后，是否允许前端直接展示“可重试”入口。

建议规则：

- 暂时性执行失败：`true`
- 注册超时：`true`
- 明显配置错误：`false`
- 参数校验错误：`false`
- 绑定冲突：默认 `false`，需人工确认或 rebind

## 第一阶段接口建议

### 1. 稳定查询接口

建议新增或冻结以下查询接口：

- `GET /api/nm/v1/nodes`
- `GET /api/nm/v1/nodes/:node_id`
- `GET /api/nm/v1/nodes/status:batch`
- `GET /api/nm/v1/nodes/:node_id/binding`
- `GET /api/nm/v1/install-tasks/:install_task_id`
- `GET /api/nm/v1/nodes/:node_id/install-tasks:latest`

#### `GET /api/nm/v1/nodes`

用途：返回节点列表摘要。

返回模型：`NodeSummary[]` + 分页信息。

建议支持的查询参数：

- `page`
- `page_size`
- `environment`
- `label`
- `lifecycle_state`
- `online_status`

#### `GET /api/nm/v1/nodes/:node_id`

用途：返回节点详情页聚合对象。

返回模型：`NodeDetail`。

#### `GET /api/nm/v1/nodes/status:batch`

用途：批量查询节点核心状态投影。

第一阶段当前实现为：

- `GET /api/nm/v1/nodes/status:batch?node_ids=node-1,node-2`

也就是用逗号分隔的 `node_ids` 查询参数承载批量节点 ID。

#### `GET /api/nm/v1/nodes/:node_id/binding`

用途：查询当前节点绑定关系。

返回模型应至少包含：

- `node_id`
- `agent_id`
- `binding_state`
- `first_registered_at`
- `last_handshake_at`

#### `GET /api/nm/v1/install-tasks/:install_task_id`

用途：按任务 ID 查询安装任务详情。

返回模型：`NodeInstallTaskView`。

#### `GET /api/nm/v1/nodes/:node_id/install-tasks:latest`

用途：查询节点最近一次安装任务。

返回模型：`NodeInstallTaskView | null`。

这个接口非常关键，因为很多前端详情页首先只需要“最近任务”而不是完整任务历史。

### 2. 过渡动作接口

第一阶段继续保留以下动作接口：

- `POST /api/nm/v1/nodes/:node_id/install`
- `POST /api/nm/v1/nodes/:node_id/rebind`
- `POST /api/nodes/node/:id/status/refresh`
- `POST /api/nm/v1/agents/sync`

其中：

- `install` 用于发起安装流程；
- `rebind` 用于显式处理绑定冲突；
- 现有 legacy `status/refresh` 仍用于手工触发状态更新；
- `sync` 用于 agent 配置同步与绑定确认；
- 第一阶段没有再新增独立的 v1 `register` 动作面。

但前端页面不应以这些动作接口的即时返回作为主展示数据来源，而应在动作成功后回到查询接口读取稳定视图。

### 3. install 动作接口约束

`POST /api/nm/v1/nodes/:node_id/install` 的返回应从“一次性安装结果”调整为“任务入口结果”。

建议返回：

- `install_task_id`
- `node_id`
- `accepted`
- `task_state`

也就是说，安装接口的职责变成：

- 接受安装请求；
- 创建安装任务；
- 返回可追踪的任务 ID；
- 不要求在一次同步请求里返回最终安装结论。

## 错误模型

### 1. 统一错误响应结构

建议第一阶段冻结如下错误响应结构：

```json
{
  "code": "INSTALL_EXECUTION_FAILED",
  "message": "failed to start agent service on remote host",
  "retryable": true,
  "details": {
    "node_id": "node-1",
    "install_task_id": "install-1",
    "current_step": "start_agent"
  }
}
```

建议字段：

- `code`: 稳定错误码
- `message`: 面向调用方的错误说明
- `retryable`: 是否可重试
- `details`: 结构化上下文

### 2. 第一阶段至少冻结的错误码

- `NODE_NOT_FOUND`
- `INSTALL_TASK_NOT_FOUND`
- `BINDING_NOT_FOUND`
- `BINDING_CONFLICT`
- `TARGET_AGENT_NOT_FOUND`
- `REBIND_TARGET_ALREADY_BOUND`
- `INSTALL_CONFIG_INVALID`
- `INSTALL_EXECUTION_FAILED`
- `AGENT_REGISTRATION_TIMEOUT`
- `STATUS_QUERY_FAILED`
- `INVALID_REBIND_REQUEST`
- `HEARTBEAT_DATALINK_NOT_READY`
- `INVALID_ARGUMENT`
- `INTERNAL_ERROR`

### 3. 错误语义要求

第一阶段要求：

- 前端不再依赖字符串模糊匹配判断错误类型；
- 同类错误必须稳定映射到同一错误码；
- `retryable` 由后端给出，不要求前端猜测；
- `details` 允许补充 `node_id`、`install_task_id`、`current_step`、`conflict_agent_id` 等上下文。

## 与现有代码的衔接方式

### 1. 不立即推翻现有动作型实现

当前 `apiserver/src/nodemanage.rs` 已有：

- `/node`
- `/node/:id`
- `/install`
- `/agent/sync`
- `/node/:id/status/refresh`

第一阶段不建议先大拆现有执行链路。

更合适的做法是：

1. 在现有服务能力之上补充稳定读模型；
2. 将安装动作返回调整为任务入口；
3. 逐步把页面依赖迁移到新查询接口；
4. 等读模型、任务模型、持久化面稳定后，再考虑更完整的声明式资源模型。

### 2. 持久化面要求

本协议虽然主要面向前后端契约，但它直接要求后端必须补齐以下持久化或读模型能力：

- `NodeInstallTask` 持久化与查询；
- `NodeDataLinkRef` 持久化或稳定读模型；
- `NodeAgentBinding` 查询面；
- 节点聚合状态读模型；
- MySQL 模式下可用的状态刷新/查询能力。

也就是说：

- 没有这些持久化/读模型能力，协议无法真正落地；
- 这也是为什么当前阶段优先级必须先补协议和持久化面，而不是先做完整 reconcile engine。

## 为什么第一阶段不强推完整声明式 NodeResource

完整 `NodeResource(metadata/spec/status)`、`generation`、`observed_generation`、reconcile engine 方向是合理的。

但当前阶段更大的阻力不在这里，而在于：

- 前端没有稳定查询契约；
- 安装任务没有稳定任务对象；
- 状态面没有批量投影；
- 错误面没有冻结；
- 持久化和读模型尚未补齐。

因此更合理的阶段化路线是：

1. 先冻结第一阶段稳定协议；
2. 补持久化与读模型；
3. 让前后端完成第一阶段联调；
4. 再把资源模型和 reconcile 机制继续演进到目标态。

## 与目标态架构的兼容关系

虽然本文不是完整声明式资源协议，但它有意为目标态做兼容铺垫：

- `lifecycle_state` / `install_phase` / `binding_state` / `online_status` 已经拆分状态轴；
- `NodeInstallTask` 已经成为独立任务对象；
- `NodeDetail` 已经采用聚合对象形式；
- 错误码已经具备产品级稳定性；
- 动作接口被降级为过渡入口而不是主读协议。

因此，后续升级时可以沿着以下路径继续演进：

1. 将 `node` 字段进一步拆成 `metadata/spec/status`；
2. 引入 `generation/observed_generation`；
3. 用更正式的控制器循环替代显式 refresh 触发；
4. 将安装、绑定、状态修正都纳入更完整的 reconcile engine。

## 第一阶段明确不做的事情

为了避免协议一次性膨胀，以下内容不纳入本文冻结范围：

- 完整声明式 `NodeResource` 资源协议；
- `generation/observed_generation` 字段语义冻结；
- 通用 reconcile DSL 或控制器框架；
- 安装任务完整历史筛选与复杂审计检索协议；
- 多类运行态指标统一协议；
- 复杂批量纳管与批量回滚协议。

## 建议的后续实现顺序

本文冻结后，建议实现顺序如下：

1. 补 `NodeInstallTask` 持久化与查询；
2. 补 `NodeDataLinkRef` 持久化或稳定读模型；
3. 补 `NodeAgentBinding` 查询接口；
4. 补 `NodeSummary` / `NodeDetail` / `NodeStatusBatchItem` DTO 与查询接口；
5. 将安装接口返回改为任务入口结果；
6. 统一错误码与错误响应结构；
7. 再评估更窄版的状态推进逻辑或 reconcile 能力。

## 参考文档

本文在以下文档基础上收敛：

- `doc/nodemanage概述.md`
- `doc/nodemanage详细设计.md`
- `doc/nodemanage安装制品模型设计.md`
- `doc/datalink-engine-api契约.md`
- `apiserver/src/nodemanage.rs`
- `nodemanage/src/protocol.rs`

后续若 `rsagent` 注册协议、heartbeat datalink 协议、query-engine 查询模型发生变化，本文应同步校对，避免前后端协议与跨组件协议漂移。
