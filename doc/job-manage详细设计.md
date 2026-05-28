# job-manage 详细设计

## 文档目标

这篇文档用于在 `doc/job-manage概述.md` 的基础上，把 `job-manage` 从“组件概述”细化为“可执行的详细设计草案”。

本文重点回答以下问题：

- `job-manage` 在节点管理体系中的明确职责是什么；
- `job-manage` 与 `nodemanage`、`rsagent`、`query-engine` 的边界如何稳定；
- 第一阶段最小任务模型应该如何定义；
- `jm ↔ rsagent` 的任务通信主线如何组织；
- 作业、节点下发记录、执行结果、状态机如何建模；
- 第一阶段哪些内容应冻结，哪些内容仍然是 TBD。

本文是第一阶段详细设计文档，不等同于最终 API 契约、数据库 schema 或代码实现的冻结版本。凡是现有仓库文档尚未证明的内容，本文会明确标注为 **Assumption** 或 **TBD**。

## 组件定位

`job-manage` 的定位保持与概述文档一致：

> `job-manage` 是面向纳管节点的远程任务编排与执行管理服务。

展开后，`job-manage` 在体系中的角色应收敛为：

- 作业创建入口；
- 节点选择与节点级调度入口；
- 任务下发协调者；
- 执行状态跟踪者；
- 结果聚合与审计记录维护者。

它不是：

- 节点纳管平台；
- 节点状态解释服务；
- 节点侧执行器；
- 数据链路元数据管理服务。

## 设计目标

`job-manage` 第一阶段建议先把“脚本下发执行闭环”做扎实，而不是一开始就把所有任务类型和复杂调度能力一次铺开。

因此本文的设计目标是：

1. 让用户可以创建脚本作业或命令执行作业；
2. 让作业可以从 `nodemanage` 获取目标节点范围；
3. 让作业在下发前基于 `nodemanage` 提供的节点状态做最小执行前检查；
4. 让 `job-manage` 通过与 `rsagent` 的通信通道下发脚本任务或命令任务；
5. 让 `rsagent` 在节点侧执行脚本或命令并回传结果；
6. 让 `job-manage` 统一维护作业级和节点级状态，并对外返回聚合结果与节点级结果。

## 与其他组件的边界

### 与 nodemanage 的边界

`nodemanage` 负责：

- 节点主体建模；
- 节点纳管；
- 节点标签、分组、基础管理态；
- 节点绑定与节点生命周期。

`job-manage` 只消费 `nodemanage` 提供的节点选择范围，不自己维护节点主体真相。

换句话说：

- 节点是不是平台内合法可选节点，由 `nodemanage` 决定；
- 这些节点上到底要执行什么任务，由 `job-manage` 决定。

### 与 query-engine 的边界

第一阶段里，`job-manage` 不应直接依赖 `query-engine` 作为主查询面。

更合理的边界是：

- `query-engine` 为 `nodemanage` 提供 heartbeat 数据查询能力；
- `nodemanage` 负责按 `node_id` 维度计算节点状态；
- `job-manage` 消费 `nodemanage` 提供的节点状态结果，并据此完成执行前检查。

### 与 rsagent 的边界

第一阶段里，`job-manage` 与 `rsagent` 的边界必须明确到可落地程度：

- `job-manage` 负责创建作业、选择节点、生成脚本执行任务或命令执行任务；
- `rsagent` 负责通过约定通信通道接收任务，在节点侧执行脚本或命令，并回传结果；
- `job-manage` 拥有任务编排、任务级状态、结果聚合和审计记录；
- `rsagent` 不应被扩成完整 job runtime，只实现第一阶段所需的最小脚本执行承接能力。

### 与 datalink-engine 的边界

`job-manage` 第一阶段不直接依赖 `datalink-engine` 作为主交互面。

如果后续任务结果本身也需要被建模为数据链路，那是后续扩展方向；第一阶段优先打通“作业创建 → 节点下发 → 执行回传”闭环。

## 第一阶段范围

## 1. 最小任务模型

第一阶段建议先收敛成：

- **脚本任务优先**

也就是说，先把“脚本下发执行”这条主线做扎实；同时第一阶段允许把命令执行纳入同一任务体系。文件传输等能力，可以在后续继续扩到统一任务框架。

### 默认任务表达方式

当前建议采用“**直接使用 `script_content`**”作为第一阶段默认表达方式：

- 脚本任务直接携带脚本文本；
- 是否支持 `script_ref`，留待后续扩展。

这样做的原因是：

- 最利于审计“当时到底执行了什么”；
- 最容易先跑通 `jm ↔ rsagent` 主链路；
- 不需要第一阶段就引入脚本仓库依赖。

如果第一阶段支持命令执行，建议把命令执行看作脚本任务模型上的一个轻量扩展，而不是单独再造一套完全不同的协议。

## 2. 节点选择与执行前检查

`job-manage` 在下发任务前建议完成两类动作：

1. 从 `nodemanage` 获取候选节点；
2. 通过 `nodemanage` 提供的节点状态结果判断节点是否可执行。

执行前检查结果建议最少分为：

- `executable`
- `skipped_offline`
- `skipped_unhealthy`
- `skipped_missing_agent`

这里要强调：

- 这些是 `job-manage` 自己的业务判断结果；
- 但这些判断应优先基于 `nodemanage` 提供的节点状态，而不是 `job-manage` 自己重复解释 heartbeat 数据。

这一步的价值是避免：

- 对离线节点盲目下发任务；
- 让用户手动推测哪些节点当前可执行。

## 3. 任务下发与结果回收

第一阶段建议把任务下发和结果回收建模成一条稳定主线：

1. 用户创建作业；
2. `job-manage` 解析目标节点；
3. `job-manage` 做执行前检查；
4. 生成单节点脚本执行任务或命令执行任务；
5. `rsagent` 主动拉取任务；
6. 节点侧执行脚本或命令；
7. `rsagent` 主动回传执行结果；
8. `job-manage` 聚合作业级和节点级状态。

## 通信模式建议

当前建议第一阶段采用：

- `rsagent` 主动拉取任务；
- `job-manage` 返回待执行脚本任务或命令任务；
- `rsagent` 显式确认接单；
- `rsagent` 执行过程中可选上报 `running`；
- `rsagent` 执行后主动回传最终结果。

原因：

- 与 `rsagent` 当前的主动注册、主动 heartbeat、主动拉配置模型一致；
- 更适合复杂网络和节点侧部署环境；
- 第一阶段不需要先引入平台主动推送的长连接通道。

后续如需要推送控制信令、长连接或流式任务分发，统一视为后续增强项。

## 核心领域对象

## 1. Job

`Job` 是作业主对象，表示一次由平台发起的脚本执行作业或命令执行作业。

建议至少包含：

- `job_id`
- `job_name`
- `job_type`
- `script_source_type`
- `script_content`
- `args`
- `env`
- `timeout_secs`
- `created_by`
- `created_at`

### 设计约束

- 第一阶段建议至少支持 `job_type = script` 和 `job_type = command`；
- 即使命令执行进入第一阶段，也建议继续围绕“脚本文本/命令文本下发执行”这一统一主线建模；
- `script_ref` 不作为第一阶段默认能力。

## 2. JobTarget

`JobTarget` 表示一次作业面向的目标节点范围及解析结果。

建议包含：

- `job_id`
- `target_selector`
- `resolved_node_ids`
- `resolved_at`

这里的关键点是：

- 用户提交的是“选择条件”；
- 系统落地时需要生成“节点快照”；
- 避免后续节点集合变化导致作业执行目标漂移。

## 3. JobDispatch

`JobDispatch` 表示作业对单个节点的一次下发记录。

建议包含：

- `dispatch_id`
- `job_id`
- `node_id`
- `agent_id`
- `dispatch_state`
- `dispatched_at`
- `ack_at`

这个对象解决的是：

- 任务有没有下发到某个节点；
- 某个节点是否已经确认接收；
- 哪些节点还在等待执行或执行失败。

## 4. JobExecutionResult

`JobExecutionResult` 表示单节点执行结果。

建议包含：

- `dispatch_id`
- `node_id`
- `execution_state`
- `stdout`
- `stderr`
- `exit_code`
- `started_at`
- `finished_at`
- `error_message`

这个对象解决的是：

- 某个节点执行结果到底是什么；
- 失败时失败在哪；
- 用户如何回看完整输出。

## 第一阶段协议设计

## 1. 作业创建协议

### 目标

允许用户提交脚本任务或命令执行任务，并描述目标节点范围。

### 建议请求字段

- `job_name`
- `job_type`
- `script_content`
- `args`
- `env`
- `working_dir`
- `timeout_secs`
- `target_selector`

### 建议响应字段

- `job_id`
- `job_state`
- `resolved_target_count`
- `created_at`

## 2. 执行前检查协议

### 目标

在真正下发之前，确认节点是否适合执行任务。

### 输入来源

- 候选节点来自 `nodemanage`
- 节点状态来自 `nodemanage`

### 输出建议

对每个节点返回：

- `node_id`
- `precheck_state`
- `reason`

### 约束

- `job-manage` 不自己解释 heartbeat；
- 只基于 `nodemanage` 提供的节点状态做执行决策。

## 3. `jm ↔ rsagent` 任务分发协议

### 目标

让 `job-manage` 能把脚本任务或命令执行任务稳定交给 `rsagent`，并让 `rsagent` 能稳定回传结果。

具体接口、状态约束与协议对象映射，见 `doc/job-manage 与 rsagent 通信协议.md`。

### 通信主线建议

当前建议第一阶段采用：

1. `rsagent` 主动拉取待执行任务；
2. `job-manage` 返回当前节点可执行任务；
3. `rsagent` 显式确认接单；
4. `rsagent` 节点侧执行脚本或命令；
5. `rsagent` 可选回传 `running` 状态；
6. `rsagent` 主动回传最终执行状态与结果。

### 最小任务字段建议

- `task_id`
- `job_id`
- `task_type`
- `script_content`
- `args`
- `env`
- `working_dir`
- `timeout`
- `issued_at`

其中：

- `task_type` 第一阶段建议至少支持 `script` 和 `command`；
- 当 `task_type = script` 时，`script_content` 表示脚本文本；
- 当 `task_type = command` 时，`script_content` 可以承载命令文本或统一的可执行文本内容；
- `script_ref` 不作为第一阶段默认能力。

### 结果回传字段建议

- `task_id`
- `job_id`
- `execution_state`
- `stdout`
- `stderr`
- `exit_code`
- `started_at`
- `finished_at`
- `error_message`

## 状态机建议

### 1. Job 状态

- `pending`
- `dispatching`
- `running`
- `partially_succeeded`
- `succeeded`
- `failed`
- `cancelled`

### 2. Dispatch 状态

- `pending`
- `dispatched`
- `acknowledged`
- `running`
- `succeeded`
- `failed`
- `timeout`
- `skipped`

### 3. 执行前检查状态

- `executable`
- `skipped_offline`
- `skipped_unhealthy`
- `skipped_missing_agent`

这些状态名是当前详细设计里的建议集合；是否完全采用这些名称，可以在后续 API 契约或实现设计里继续收敛。

## 结果模型建议

第一阶段建议至少支持两层结果视图：

### 1. 节点级结果

每个节点返回：

- 成功/失败/超时/跳过；
- 完整 stdout；
- 完整 stderr；
- exit_code；
- 错误信息；
- 开始和结束时间。

### 2. 作业级结果

作业聚合层建议至少给出：

- 总节点数；
- 成功数；
- 失败数；
- 跳过数；
- 当前整体状态；
- 最近更新时间。

## 第一阶段约束

第一阶段建议只锁定以下最小闭环：

1. 创建脚本作业；
2. 支持命令执行作业；
3. 从 `nodemanage` 选择节点；
4. 通过 `nodemanage` 提供的节点状态做执行前检查；
5. 通过 `rsagent` 通道下发脚本或命令；
6. 节点侧执行并回传完整结果；
7. 在 `job-manage` 中聚合状态与结果。

## 第一阶段明确不展开的内容

以下能力建议暂不纳入第一阶段冻结范围：

1. 长连接推送调度；
2. 文件传输协议细节；
3. 复杂失败重试策略；
4. DAG 任务编排；
5. 插件化任务执行器；
6. 作业结果与数据链路建模；
7. 多租户与复杂权限体系；
8. 完整取消/中断协议（第一阶段只保留空实现或占位入口）；
9. 长期归档或链路化结果管理。

## Rust 落地结构建议

这一节不是强制实现结构，只是为了让后续实现和领域模型更容易对齐。

建议至少拆成：

- `domain/`
  - `job.rs`
  - `job_target.rs`
  - `job_dispatch.rs`
  - `job_result.rs`
- `service/`
  - `job_service.rs`
  - `dispatch_service.rs`
  - `precheck_service.rs`
  - `result_service.rs`
- `integration/`
  - `nodemanage_client.rs`
  - `rsagent_task_channel.rs`
- `api/`
- `state/`

## Assumptions / TBD 汇总

以下内容在当前仓库文档中尚未被正式定义，因此在本文中统一视为默认设计方向：

1. `script_ref` 是否在后续阶段进入正式能力面；
2. `jm ↔ rsagent` 任务拉取接口的最终路径和返回格式；
3. `JobTarget.target_selector` 的最终表达格式；
4. 第一阶段命令执行与脚本执行是否完全共用同一 payload 结构；
5. 失败重试和超时补偿策略在后续阶段如何展开；
6. 作业取消和中断协议的正式实现方式（第一阶段只留空实现）；
7. 完整 stdout/stderr 的存储上限与截断策略；
8. 是否对作业结果做长期归档或链路化管理（第一阶段不做）。

## 后续拆分建议

如果继续细化 `job-manage`，建议后续拆出以下文档：

1. `job-manage API 契约.md`
2. `job-manage 与 rsagent 通信协议.md`
3. `job-manage 作业模型与状态机.md`
4. `job-manage 脚本任务执行设计.md`

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/job-manage概述.md`
- `doc/rsagent详细设计.md`
- `doc/nodemanage详细设计.md`
- `doc/query-engine概述.md`

后续如果 `rsagent` 的任务协议或 `nodemanage` 的节点模型继续演进，本文中的 `jm` 详细设计也需要同步更新，避免三者之间的接口边界发生漂移。
