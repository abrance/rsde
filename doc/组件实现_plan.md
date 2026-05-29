# 组件实现计划

> **适用范围**：本计划用于明确 `nodemanage`、`query-engine`、`job-manage`、`rsagent` 四个组件当前实现进度，以及把它们推进到“可以被明确称为第一版完成”的后续落地步骤。

## 1. 结论先说

截至当前 worktree（`feature/node-runtime-v1`）的真实代码状态：

| 组件 | 当前结论 | 说明 |
| --- | --- | --- |
| `nodemanage` | **第一版最小闭环已实现** | 已有节点 CRUD、agent 注册、安装编排、heartbeat 刷新、状态计算、apiserver 接入。 |
| `query-engine` | **第一版最小闭环已实现** | 已有 heartbeat 查询、`result_table_name`/`data_link_id` 两种查法、与 `datalink-engine` 协作。 |
| `job-manage` | **仅实现最小 precheck 子集** | 当前只有基于 `nodemanage` 节点状态的 `PrecheckService`，尚未进入真正的 job / dispatch / protocol / result 闭环。 |
| `rsagent` | **尚未实现为代码组件** | 当前只有详细设计、协议文档，以及 `nodemanage` 侧安装/注册接入点；仓库里还没有 `rsagent` workspace crate。 |

因此，**不能说 `nm + qe + rsagent + jm` 四个组件都已经完成第一版**。

更准确的说法是：

- `nm`、`qe`：已经完成第一版最小实现；
- `jm`：完成了一个很薄的第一阶段子切片；
- `rsagent`：还停留在“设计 + 安装侧集成点”阶段。

---

## 2. 当前实现事实（按组件）

### 2.1 nodemanage

当前已经落地的内容：

1. 节点主体管理与 CRUD
2. `agent/register` 注册主线
3. SSH 安装编排与等待注册
4. heartbeat datalink bootstrap
5. 通过 `query-engine` 刷新节点在线状态
6. `apiserver` 路由接入

关键代码依据：

- `nodemanage/src/service.rs`
- `nodemanage/src/bootstrap.rs`
- `nodemanage/src/protocol.rs`
- `apiserver/src/nodemanage.rs`
- `nodemanage/tests/service.rs`
- `apiserver/tests/nodemanage_routes.rs`

当前仍未完成但不阻塞“最小第一版”的部分：

- 更细的状态语义（例如 stale / unknown / degraded）
- MySQL 路径下的 query refresh 真正接通
- 更完整的安装包发布链路

### 2.2 query-engine

当前已经落地的内容：

1. `HeartbeatStore` 抽象
2. `QueryEngine` 最小查询实现
3. 按 `result_table_name` 查询 heartbeat
4. 按 `data_link_id` 查询 heartbeat
5. heartbeat 数据解析与错误路径测试

关键代码依据：

- `query-engine/src/service.rs`
- `query-engine/src/memory.rs`
- `query-engine/tests/heartbeat_query.rs`

当前仍未完成但不阻塞“最小第一版”的部分：

- 真正的外部存储后端接入（现在主要是内存路径验证）
- 批量查询接口
- 通用 QueryTarget / ResolvedMetadata 抽象完全落地

### 2.3 job-manage

当前已经落地的内容非常少，只有：

1. `PrecheckService`
2. 基于 `nodemanage` 节点状态的可执行性判断
3. `NodePrecheck` 返回模型
4. 对在线 / 离线 / 节点不存在的测试覆盖

关键代码依据：

- `job-manage/src/service.rs`
- `job-manage/src/models.rs`
- `job-manage/src/error.rs`
- `job-manage/tests/precheck.rs`

当前明确**还没有**的内容：

1. `Job` 主对象
2. `JobTarget`
3. `JobDispatch`
4. `poll / ack / status / result` 协议实现
5. `apiserver` 接入
6. 节点级结果聚合
7. 作业状态机

因此 `job-manage` 现在还不能单独称为“完整第一版”。

### 2.4 rsagent

当前代码仓库里的真实状态是：

1. 有设计文档：`doc/rsagent详细设计.md`
2. 有 `jm ↔ rsagent` 协议文档：`doc/job-manage 与 rsagent 通信协议.md`
3. 有 `nodemanage` 侧安装器与注册等待逻辑：`nodemanage/src/bootstrap.rs`
4. 有 `AgentRegistration` 协议对象：`nodemanage/src/protocol.rs`

但当前明确**没有**的内容：

1. 没有 `rsagent/` crate
2. 没有 agent 主程序 `main.rs`
3. 没有本地配置加载实现
4. 没有 heartbeat 上报 runtime
5. 没有配置拉取 runtime
6. 没有任务拉取 / ACK / 结果回传实现

当前 workspace `Cargo.toml` 也没有 `rsagent` member，因此它仍然属于“未实现为组件代码”的状态。

---

## 3. 第一版完成标准（建议冻结）

为了避免后续反复讨论“算不算第一版”，建议统一采用下面的完成标准。

### 3.1 nodemanage 第一版完成标准

满足以下条件即可认为第一版完成：

1. 节点可创建、查询、更新、删除
2. 节点可安装 rsagent，并等待注册
3. 节点可通过 heartbeat 刷新在线状态
4. heartbeat datalink 在启动时自动 bootstrap
5. `apiserver` 对外暴露稳定节点管理 API

按这个标准，**`nodemanage` 当前已满足**。

### 3.2 query-engine 第一版完成标准

满足以下条件即可认为第一版完成：

1. 可以根据 `data_link_id` 解析 heartbeat 查询目标
2. 可以返回统一 heartbeat 查询结果
3. 能为 `nodemanage` 状态刷新提供稳定输入
4. 关键错误路径有测试覆盖

按这个标准，**`query-engine` 当前已满足最小版**。

### 3.3 job-manage 第一版完成标准

建议冻结为：

1. 用户可创建一个最小作业（script / command）
2. 可从 `nodemanage` 获取目标节点并做 precheck
3. 可生成单节点 dispatch 记录
4. 可通过 `poll -> ack -> result` 与 `rsagent` 完成单节点执行闭环
5. 可维护 job / dispatch 状态并聚合结果
6. 可通过 `apiserver` 提供最小 API

按这个标准，**`job-manage` 当前明显未满足**。

### 3.4 rsagent 第一版完成标准

建议冻结为：

1. 是一个独立 workspace crate
2. 能加载安装后生成的本地配置
3. 启动后可向 `nodemanage` 注册
4. 能按 heartbeat datalink 周期性上报 heartbeat
5. 能主动拉取 job task
6. 能 ACK、执行脚本/命令，并回传最终结果

按这个标准，**`rsagent` 当前未满足**。

---

## 4. 总体实现顺序建议

下一阶段不建议再平均推进四个组件，而应按依赖关系推进：

### Phase A：冻结 nm / qe 最小版本，不再大扩

目标：

- 把 `nodemanage` / `query-engine` 视为可依赖底座；
- 后续只做必要缺陷修补，不再在这一轮大规模扩边界。

本阶段只保留两类工作：

1. 必要的错误语义细化
2. 为 `jm` / `rsagent` 接入补最小扩展点

### Phase B：先把 rsagent 作为代码组件创建出来

这是当前最大的真实缺口。

原因：

- `job-manage` 第一版必须依赖一个真正可执行的 agent；
- 如果没有 `rsagent` crate，`jm ↔ rsagent` 协议永远只能停留在文档层；
- 先补 `jm` 而不补 `rsagent`，只能继续写空壳。

### Phase C：在 rsagent 存在后，再完成 jm 第一版闭环

顺序必须是：

1. 先有 agent 端运行时；
2. 再补 job-manage 协议服务端；
3. 再打通单任务闭环。

---

## 5. 组件级落地计划

## 5.1 nodemanage / query-engine：收尾计划

### 目标

把 `nm` / `qe` 稳定为后续组件可依赖底座。

### 建议任务

1. 保持 `data_link_id`-aware heartbeat refresh 不再回退
2. 只补必要的错误路径测试
3. 如需对外 API 语义更清晰，再细化 stale/missing heartbeat-link 错误

### 涉及文件

- `nodemanage/src/service.rs`
- `apiserver/src/nodemanage.rs`
- `query-engine/src/service.rs`
- 相关 tests

### 完成判断

- 无新的大功能扩张
- 能稳定支撑 `rsagent` 注册与 `job-manage` precheck

## 5.2 rsagent：第一版真正实现计划

### 目标

新增一个真正可运行的 `rsagent` workspace crate，先打通注册 + heartbeat + 最小任务拉取/执行/结果回传主线。

### 推荐新增文件/目录

- `rsagent/Cargo.toml`
- `rsagent/src/main.rs`
- `rsagent/src/lib.rs`
- `rsagent/src/config.rs`
- `rsagent/src/register.rs`
- `rsagent/src/heartbeat.rs`
- `rsagent/src/task_poll.rs`
- `rsagent/src/execute.rs`
- `rsagent/tests/*`

同时需要修改：

- 根 `Cargo.toml`：把 `rsagent` 加入 workspace members
- `common/config`：如需共享 agent 本地配置模型，可新增 `rsagent.rs`

### 第一版最小任务拆分

#### Task R1：创建 rsagent crate 骨架

- 新建 crate 并加入 workspace
- 提供最小 `main.rs`
- 提供配置加载与启动入口

#### Task R2：实现注册主线

- 读取本地配置
- 生成 `agent_id` / 节点身份材料
- 向 `nodemanage` 注册

#### Task R3：实现 heartbeat 上报主线

- 加载 `data_link_id`
- 周期性生成 heartbeat 事实
- 向约定上报端发送数据

#### Task R4：实现最小任务协议客户端

- 向 `job-manage` 执行 `poll`
- 支持 `ack`
- 支持最终 `result` 回传

#### Task R5：实现脚本 / command 执行器

- 接收 `script_content`
- 本地执行
- 收集 `stdout/stderr/exit_code`
- 回传执行结果

### 第一版完成标志

- `rsagent` 是独立 crate
- 能真实启动
- 能注册
- 能 heartbeat
- 能执行一条从 `job-manage` 拉到的最小任务

## 5.3 job-manage：第一版闭环计划

### 目标

在 `rsagent` crate 存在后，把 `job-manage` 从“precheck 小工具”推进到“最小作业编排器”。

### 建议新增文件

- `job-manage/src/job.rs`
- `job-manage/src/dispatch.rs`
- `job-manage/src/protocol.rs`
- `job-manage/src/repository.rs`
- `job-manage/src/api.rs` 或由 `apiserver` 直接装配路由
- `job-manage/tests/*`

### 第一版最小任务拆分

#### Task J1：补领域对象

- `Job`
- `JobTarget`
- `JobDispatch`
- 状态枚举

#### Task J2：补 repository / in-memory runtime

- 能保存 job
- 能保存 dispatch
- 能按 agent/node 查询待执行 dispatch

#### Task J3：补最小服务层

- 创建 job
- 解析目标节点
- 基于 `nodemanage` 做 precheck
- 生成 dispatch

#### Task J4：补 `jm ↔ rsagent` 协议服务端

- `poll`
- `ack`
- `result`

#### Task J5：补 apiserver 接入

- 在 `apiserver/src/lib.rs` 挂载 job-manage routes
- 如需要，先在 `common/config` 增加 `job-manage` 配置块

### 第一版完成标志

- 能创建最小脚本作业
- 能生成单节点 dispatch
- `rsagent` 能 poll 到任务并执行
- `job-manage` 能接收结果并聚合状态

---

## 6. 推荐里程碑

### Milestone 1：底座冻结

- `nm` / `qe` 只做必要修补
- 明确它们已经可作为后续依赖

### Milestone 2：rsagent crate 落地

- 这是最优先的大缺口

### Milestone 3：jm ↔ rsagent 单任务闭环

- `poll -> ack -> result`
- 单 agent 单活动任务

### Milestone 4：apiserver 接总入口

- 挂载 `job-manage` API
- 让平台端可创建 job / 查询 job 结果

---

## 7. 建议的下一步执行顺序（明确版）

如果你现在要继续实现，建议严格按下面顺序走：

1. **不要再把 `nm` / `qe` 当主战场**，它们已经足够支撑下一阶段；
2. **先创建 `rsagent` crate**，哪怕先只支持注册 + heartbeat；
3. **再给 `job-manage` 补 Job / Dispatch / Poll/Ack/Result**；
4. **最后再把 `job-manage` 接入 apiserver**。

一句话总结：

> 当前真正缺的不是继续打磨 `nm` / `qe`，而是把 **`rsagent` 从文档变成代码组件**，再据此完成 `jm` 的最小闭环。

---

## 8. 当前建议判断语句（可直接对外统一说法）

建议统一使用下面这段话描述当前状态：

> `nodemanage` 与 `query-engine` 已经完成第一版最小实现；`job-manage` 目前只完成了 precheck 子能力；`rsagent` 仍处于设计与安装侧接入阶段，尚未实现为独立代码组件。因此四个组件还不能整体称为“全部第一版完成”，下一阶段重点应放在 `rsagent` crate 落地以及 `job-manage ↔ rsagent` 最小任务闭环。`
