# job-manage 与 rsagent 通信协议

## 文档目标

这篇文档用于把 `job-manage`（`jm`）与 `rsagent` 在第一阶段的通信方式收敛成一份单独协议文档。

本文重点回答以下问题：

- 第一阶段 `jm ↔ rsagent` 到底采用什么通信模型；
- `job-manage` 与 `rsagent` 分别负责什么；
- 任务拉取、显式 ACK、运行状态上报、结果回传的接口如何组织；
- `task_id`、`dispatch_state`、`execution_state` 之间如何映射；
- 第一阶段哪些内容需要冻结，哪些仍然是后续扩展项。

本文是第一阶段协议设计文档，不等同于最终 API 路径、HTTP 鉴权实现或 Rust 代码结构的冻结版本。凡是当前仓库文档尚未证明的内容，本文会明确标注为 **Assumption** 或 **TBD**。

## 协议定位与边界

在节点管理体系中：

- `job-manage` 负责创建作业、选择节点、生成单节点任务、维护调度状态、聚合执行结果；
- `rsagent` 负责在节点侧拉取任务、显式确认接单、执行任务并回传结果；
- `nodemanage` 负责提供合法节点范围和节点状态，不直接参与任务协议；
- `query-engine` 负责为 `nodemanage` 提供 heartbeat 数据查询能力，不直接参与任务协议。

因此，本文讨论的是“**任务被分配给某个 agent 之后**”的通信问题，而不是：

- 作业创建 API；
- 节点筛选 API；
- 执行前检查 API；
- 文件传输协议；
- 取消中断协议；
- 长连接推送协议。

## 第一阶段协议结论

第一阶段建议冻结以下结论：

1. 协议模式采用：`poll -> ack -> optional status(running) -> result`；
2. 由 `rsagent` 主动拉取任务，不采用平台主动推送；
3. 单次 `poll` 最多返回一个任务；
4. 单个 `rsagent` 第一阶段建议只并发执行一个活动任务；
5. 任务类型第一阶段支持 `script` 和 `command`；
6. 任务内容统一由 `script_content` 承载；
7. 结果侧返回完整 `stdout`、完整 `stderr`、`exit_code` 和时间戳；
8. `ack` 与 `result` 需要具备幂等语义；
9. 第一阶段不做复杂失败补偿、文件传输、流式日志和真实取消中断。

## 术语约定

### 1. Job

`Job` 是用户视角的一次作业请求，例如“在某批节点上执行一个脚本”或“在某批节点上执行一个命令”。

### 2. JobDispatch

`JobDispatch` 是 `job-manage` 内部对某个节点的一次下发记录，表示“这个 `Job` 在这个节点上的执行实例”。

### 3. Task

`Task` 是协议层暴露给 `rsagent` 的执行对象。

第一阶段建议采用：

- 一个 `JobDispatch` 对应一个 `Task`；
- 协议里统一使用 `task_id`；
- 实现层可以令 `task_id = dispatch_id`，也可以维护一对一映射。

也就是说，对 `rsagent` 而言，只需要理解“我要执行哪个 `task_id`”，不需要感知 `job-manage` 内部更复杂的数据库主键组织方式。

### 4. ACK

`ACK` 表示 `rsagent` 明确告诉 `job-manage`：

- 这个任务我已经接收；
- 这个任务确实由我来执行；
- 可以把状态从“已下发”推进到“已确认接单”。

### 5. Result

`Result` 表示 `rsagent` 回传的最终执行结果。第一阶段只冻结“最终结果回传”，不要求先实现流式日志或增量结果同步。

## 通信模型总览

第一阶段建议采用以下时序：

1. 用户在 `job-manage` 创建 `Job`；
2. `job-manage` 解析目标节点，并基于 `nodemanage` 提供的节点状态完成执行前检查；
3. `job-manage` 为每个可执行节点创建 `JobDispatch`；
4. 某个节点上的 `rsagent` 主动调用 `poll` 拉取任务；
5. `job-manage` 返回分配给该节点的单个 `Task`，并将该下发记录推进到 `dispatched`；
6. `rsagent` 调用 `ack` 显式确认接单，`job-manage` 将其推进到 `acknowledged`；
7. `rsagent` 开始执行后，可选调用 `status` 上报 `running`；
8. `rsagent` 执行完成后，通过 `result` 回传最终结果；
9. `job-manage` 更新节点级结果，并聚合作业级状态。

这条主线的核心价值是：

- 把“下发成功”和“agent 真正接单”区分开；
- 让 `dispatch_state` 更清楚；
- 保持与 `rsagent` 已有的主动注册、主动 heartbeat、主动拉配置模型一致。

## 传输与通用约束

### 1. 传输方式

第一阶段建议采用：

- `HTTP(S)` 作为传输层；
- `JSON` 作为请求与响应编码；
- `UTF-8` 作为文本编码。

这是为了优先降低实现复杂度，并与现有服务形态保持一致。

### 2. 时间格式

时间字段建议统一使用 `RFC3339 / ISO8601` 时间戳，例如：

- `2026-05-28T10:15:30Z`

### 3. 认证方式

第一阶段协议不能默认匿名调用。

本文建议冻结以下原则：

- `job-manage` 必须校验 `agent_id`、`node_id` 与安装时发放的认证材料是否匹配；
- 认证材料建议通过 HTTP Header 传递；
- 具体采用 bearer token、签名材料还是节点证书，当前统一标记为 **TBD**。

### 4. 幂等要求

第一阶段建议：

- `ack` 幂等；
- `result` 幂等；
- 幂等判断建议至少基于 `task_id + agent_id`。

这样可以容忍：

- agent 网络重试；
- 请求超时后的重复提交；
- result 回传成功但响应丢失的场景。

### 5. 空任务语义

`poll` 返回空任务不是错误，而是正常业务语义。

例如：

- 当前没有可执行任务；
- 当前 agent 已存在活动任务，不再分配新任务；
- 当前节点没有匹配的待执行任务。

## 并发模型建议

第一阶段建议采用“**单 agent 单活动任务**”模型：

- 一个 `rsagent` 同一时刻最多执行一个活动任务；
- 在已有任务进入终态之前，`job-manage` 不再向该 agent 分配第二个任务。

这样做的原因是：

- 协议更简单；
- 节点侧执行器更容易实现；
- 状态跟踪、排障和结果归因都更直接；
- 后续如需扩展并发度，可以在协议不大改的情况下增加调度能力。

如果 agent 已经存在未终态任务，`poll` 的建议行为是：

- 若该任务已被下发但尚未 `ack`，优先返回同一个 `task_id`；
- 若该任务已经 `acknowledged` 或 `running`，返回空任务即可，不分配新的任务。

## 协议对象与状态映射

### 1. JobDispatch 状态建议

- `pending`
- `dispatched`
- `acknowledged`
- `running`
- `succeeded`
- `failed`
- `timeout`
- `skipped`

### 2. 状态推进建议

- `pending -> dispatched`
  - 条件：`poll` 返回了一个 `Task`
- `dispatched -> acknowledged`
  - 条件：收到对应 `ack`
- `acknowledged -> running`
  - 条件：收到 `status(running)`，或由实现层在结果回传时补推导
- `running -> succeeded|failed|timeout`
  - 条件：收到终态 `result`
- `pending -> skipped`
  - 条件：执行前检查不通过

### 3. 关于未 ACK 场景

如果发生以下情况：

- `job-manage` 已返回任务；
- `rsagent` 尚未发送 `ack`；
- agent 进程中断或网络抖动；

则该任务会停留在 `dispatched`。

第一阶段建议：

- 不引入复杂自动补偿；
- 保留这个中间态，供后续人工处理或增强调度器处理；
- 当同一 agent 再次 `poll` 时，可以返回同一个未完成 `task_id`，避免新旧任务混淆。

## 协议接口设计

## 1. 拉取任务：`poll`

### 目标

让 `rsagent` 主动从 `job-manage` 拉取当前节点可执行的单个任务。

### 建议路径

`POST /api/job-manage/v1/agent/tasks/poll`

### 建议请求字段

- `agent_id`
- `node_id`
- `agent_version`
- `supported_task_types`
- `poll_at`

其中：

- `supported_task_types` 第一阶段建议至少支持 `script` 和 `command`；
- `agent_version` 便于后续做兼容性判断和灰度控制。

### 建议响应字段

无任务时：

- `has_task`
- `next_poll_after_secs`

有任务时：

- `has_task`
- `task`
- `next_poll_after_secs`

### `task` 建议字段

- `task_id`
- `job_id`
- `task_type`
- `script_content`
- `args`
- `env`
- `working_dir`
- `timeout_secs`
- `issued_at`

### 设计约束

- 单次 `poll` 最多返回一个任务；
- `task_type = script` 时，`script_content` 表示脚本文本；
- `task_type = command` 时，`script_content` 表示命令文本；
- `script_ref` 不作为第一阶段默认能力；
- `poll` 返回空任务是正常语义，不应视为错误。

## 2. 显式接单：`ack`

### 目标

让 `rsagent` 明确告诉 `job-manage`：我已经接收并接受执行这个任务。

### 建议路径

`POST /api/job-manage/v1/agent/tasks/{task_id}/ack`

### 建议请求字段

- `agent_id`
- `node_id`
- `ack_at`

### 建议响应字段

- `task_id`
- `acknowledged`
- `accepted_at`

### 设计约束

- `ack` 必须幂等；
- `ack` 建议在本地真正开始执行前发送；
- 收到 `ack` 后，`job-manage` 将对应 `JobDispatch` 推进到 `acknowledged`。

## 3. 运行状态上报：`status`

### 目标

让 `rsagent` 在第一阶段可选上报“任务已开始执行”。

### 建议路径

`POST /api/job-manage/v1/agent/tasks/{task_id}/status`

### 建议请求字段

- `agent_id`
- `node_id`
- `execution_state`
- `reported_at`
- `started_at`

### 设计约束

- 第一阶段 `execution_state` 建议只冻结 `running`；
- 这是一个可选接口，不是协议最小闭环的强制前提；
- 如果第一阶段暂不实现该接口，`job-manage` 也可以在收到终态结果时直接推进终态。

## 4. 最终结果回传：`result`

### 目标

让 `rsagent` 把节点侧的最终执行结果完整回传给 `job-manage`。

### 建议路径

`POST /api/job-manage/v1/agent/tasks/{task_id}/result`

### 建议请求字段

- `agent_id`
- `node_id`
- `execution_state`
- `stdout`
- `stderr`
- `exit_code`
- `started_at`
- `finished_at`
- `error_message`

可选补充：

- `duration_ms`

### 设计约束

- 第一阶段 `execution_state` 终态建议至少支持：`succeeded`、`failed`、`timeout`；
- `stdout` 与 `stderr` 按语义要求应完整回传；
- 具体的存储上限、截断策略和压缩策略，当前统一标记为 **TBD**；
- `result` 必须幂等；
- 收到终态 `result` 后，`job-manage` 应更新 `JobExecutionResult`，并据此更新作业聚合状态。

## 错误处理建议

### 1. 请求错误

对于以下问题，建议返回明确错误：

- `agent_id` 与 `node_id` 不匹配；
- 任务不存在；
- 任务不属于当前 agent；
- 请求字段缺失或格式错误；
- 认证失败。

### 2. agent 重试

如果 agent 发生超时或网络抖动：

- `ack` 可以重复提交；
- `result` 可以重复提交；
- `job-manage` 应以幂等方式吸收重复请求。

### 3. result 先于 status

第一阶段允许出现：

- 未发送 `status(running)`；
- 直接提交终态 `result`。

在这种情况下，`job-manage` 可以直接把对应下发记录推进到终态，不要求先经过可观测的 `running` 持久化阶段。

## 第一阶段明确不展开的内容

以下内容建议不纳入第一阶段冻结范围：

1. 长连接推送调度；
2. 文件传输协议；
3. 任务取消和节点侧中断协议；
4. 流式日志回传；
5. 复杂失败重试与租约补偿；
6. 多任务并发调度；
7. 结果归档、链路化建模和后续分析通道。

## Assumptions / TBD 汇总

以下内容当前仍然保留为后续待定：

1. HTTP Header 里的具体认证材料格式；
2. `task_id` 是否直接等于 `dispatch_id`；
3. `stdout/stderr` 的大小上限、截断和压缩策略；
4. `status` 接口是否在第一阶段实现为必选；
5. 未 ACK 任务在后续增强版本中的自动回收与补发策略；
6. 后续是否引入 `lease_expire_at`、`delivery_attempt`、`retry_count` 等调度字段。

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/job-manage详细设计.md`
- `doc/rsagent详细设计.md`
- `doc/job-manage概述.md`
- `doc/rsagent概述.md`

后续如果 `job-manage` 的调度模型或 `rsagent` 的执行器模型继续演进，本文中的协议设计也需要同步更新，避免设计边界发生漂移。
