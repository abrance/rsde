# JobManage 后端接口待实现清单

本文用于记录当前 `JobManage` 前端从 mock 数据切换到真实后端时，仍缺失的后端接口能力，以及每个接口对应的职责说明。

## 一、背景说明

当前仓库中：

- 前端 `JobManage` 页面已经完成基础界面实现；
- 页面具备作业列表、详情展示、预检、重试、取消、刷新等前端交互；
- 但这些能力目前仍然基于前端 mock 数据；
- `apiserver` 中尚未挂载 `job-manage` 对应的 HTTP 路由。

因此，要把当前页面真正接入后端，需要补齐一组 `job-manage` 相关接口。

---

## 二、必须补齐的接口

### 1. 获取作业列表

- **接口建议**：`GET /api/job-manage/v1/jobs`
- **用途说明**：
  - 为 `JobManage` 页面首屏列表提供真实数据；
  - 为页面“刷新数据”操作提供后端来源；
  - 支持前端从 mock 数据切换到真实作业列表。
- **建议支持的能力**：
  - 按状态过滤；
  - 后续可扩展分页、关键字检索。

### 2. 获取作业详情

- **接口建议**：`GET /api/job-manage/v1/jobs/:job_id`
- **用途说明**：
  - 为右侧详情面板提供完整作业信息；
  - 展示最近执行摘要、错误信息、节点执行状态、预检结果；
  - 支撑前端从“列表态”切换到“详情态”。

### 3. 执行前检查（Precheck）

- **接口建议**：`POST /api/job-manage/v1/jobs/:job_id/precheck`
- **用途说明**：
  - 对当前作业目标节点执行执行前检查；
  - 根据 `nodemanage` 提供的节点状态快照判断节点是否可执行；
  - 返回每个节点的预检结果，例如：
    - `executable`
    - `skipped_offline`
    - `skipped_unhealthy`
    - `skipped_missing_agent`
- **补充说明**：
  - 该接口是当前前端“执行预检”按钮对应的核心后端能力；
  - `job-manage` 应消费 `nodemanage` 的节点状态结果，而不是自己重复解释 heartbeat。

### 4. 重试作业

- **接口建议**：`POST /api/job-manage/v1/jobs/:job_id/retry`
- **用途说明**：
  - 对失败作业或已取消作业发起重新执行；
  - 触发重新解析目标节点、重新预检或重新下发；
  - 为前端“重试作业”按钮提供真实动作。

### 5. 取消作业

- **接口建议**：`POST /api/job-manage/v1/jobs/:job_id/cancel`
- **用途说明**：
  - 对 `pending` 或 `running` 状态的作业执行取消；
  - 将作业状态更新为取消中或已取消；
  - 为前端“取消作业”按钮提供真实动作。
- **补充说明**：
  - 第一阶段可以先实现“标记取消”；
  - 是否支持真正中断节点侧执行，可视协议能力后续增强。

---

## 三、建议补齐但可后置的接口

### 6. 创建作业

- **接口建议**：`POST /api/job-manage/v1/jobs`
- **用途说明**：
  - 用于从前端新建脚本作业或命令作业；
  - 对应文档中 `job_name`、`job_type`、`script_content`、`args`、`env`、`timeout_secs`、`target_selector` 等字段。
- **当前状态**：
  - 现有前端页面还没有接“创建作业”表单；
  - 但如果下一步继续扩展，这将是第一优先级接口之一。

### 7. 节点筛选/候选节点查询

- **接口建议**：
  - 复用现有 `GET /api/nodes/node`
  - 或新增更适合 JobManage 的筛选接口
- **用途说明**：
  - 用于作业创建时选择目标节点；
  - 支持按标签、环境、节点状态等条件筛选候选节点。
- **补充说明**：
  - 当前 `nodemanage` 已有节点列表接口；
  - 但如果要支持更细粒度筛选或批量预检，现有能力可能不够。

### 8. 批量节点状态刷新/批量预检

- **接口建议**：后续按需要新增批量接口
- **用途说明**：
  - 避免前端逐节点调用刷新；
  - 提高作业创建与预检阶段的效率；
  - 更适合真实的批量任务场景。

---

## 四、当前已存在、可复用的接口

以下接口已存在于 `nodemanage`，可作为 `job-manage` 的依赖能力使用：

### 1. 获取节点列表

- `GET /api/nodes/node`
- 用途：获取候选节点集合。

### 2. 获取节点详情

- `GET /api/nodes/node/:id`
- 用途：获取单节点详情与当前状态。

### 3. 刷新单节点状态

- `POST /api/nodes/node/:id/status/refresh`
- 用途：刷新节点状态，供预检前使用。

---

## 五、当前不需要补的内容

以下内容已经由前端完成，不依赖新增后端接口：

- 首页 `JobManage` 工具卡展示；
- 顶部导航中的 `JobManage` 入口；
- `/job-manage` 路由页面挂载。

---

## 六、建议实现顺序

建议按以下顺序补齐后端能力：

1. `GET /api/job-manage/v1/jobs`
2. `GET /api/job-manage/v1/jobs/:job_id`
3. `POST /api/job-manage/v1/jobs/:job_id/precheck`
4. `POST /api/job-manage/v1/jobs/:job_id/retry`
5. `POST /api/job-manage/v1/jobs/:job_id/cancel`
6. `POST /api/job-manage/v1/jobs`

这样可以先把当前页面从 mock 数据切到真实读写，再继续扩展作业创建能力。

---

## 七、一句话总结

当前 `JobManage` 前端页面已经具备基础交互，但要真正投入使用，核心还缺的是 **作业列表、作业详情、预检、重试、取消** 这 5 类后端接口；节点相关能力可以先复用 `nodemanage`，作业创建与批量预检能力可作为下一阶段增强。
