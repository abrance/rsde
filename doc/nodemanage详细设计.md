# nodemanage 详细设计

## 文档目标

这篇文档用于在 `doc/nodemanage概述.md` 的基础上，把 `nodemanage` 从“组件概述”细化为“可执行的详细设计草案”。

本文优先回答以下问题：

- `nodemanage` 到底负责什么，不负责什么；
- `nodemanage` 里的核心领域对象应该如何收敛；
- 节点纳管、安装、注册、状态查询主链路如何协作；
- `nodemanage` 与 `rsagent`、`datalink-engine`、`query-engine`、`job-manage` 的边界如何稳定下来；
- 在第一阶段里，哪些设计已经足够明确，哪些仍然只能作为默认假设或 TBD。

本文是第一阶段详细设计文档，不等同于最终 API 契约、数据库 schema 或 Rust 代码结构的冻结版本。凡是现有文档尚未证明的内容，本文会明确标注为 **Assumption** 或 **TBD**。

## 术语约定

为了避免和现有项目中的其他概念混淆，本文先统一术语。

### 1. nodemanage

本文统一使用 `nodemanage` 表示该组件名称；如果其他文档中出现 `NodeManage`，视为同一组件的大小写变体。

### 2. node

本文中的 `node` 指的是“被平台纳管的一台机器实例”。

它不是：

- Kubernetes Node；
- `rsagent` 实例本身；
- 某一次任务执行单元。

**本文默认假设：`node = machine`。**

也就是说，`node` 的主体是机器，`rsagent` 是挂在机器上的代理实例，后续重装 agent 或替换 agent 时，原则上更新的是同一个节点主体，而不是重新定义一个全新的 `node`。

### 3. management state / runtime state

- **管理态**：由 `nodemanage` 自己维护的节点元信息、安装状态、注册状态、绑定关系等。
- **运行态**：由节点运行后产生的 heartbeat、最近心跳时间、在线/离线/未知等状态。

本文约定：

- 管理态归 `nodemanage`；
- 运行态查询归 `query-engine`；
- 节点状态计算与解释归 `nodemanage`；
- `nodemanage` 不直接解释底层存储查询细节，但负责把 heartbeat 查询结果转换为节点状态语义。

### 4. 安装制品域术语

本文会使用以下安装相关术语：

- `package`：安装包元数据；
- `script`：安装脚本元数据；
- `artifact profile`：一组安装制品选择规则；
- `resolution`：一次安装任务最终解析出的制品选择结果。

这里特别说明：

- `artifact profile` 仅表示安装制品域中的“选择规则对象”；
- 它与 `datalink-engine` 文档中的 `profile` 数据类型不是一个概念。

为避免歧义，后续如拆分子文档，建议优先使用 `install_profile` 这一命名，而不是单独使用 `profile`。

## 设计目标

`nodemanage` 第一阶段的目标不是做成“万能执行平台”，而是优先打通“节点被平台稳定纳管并可见状态”的主链路。

因此本文的设计目标是：

1. 把节点从静态台账提升为可纳管对象；
2. 让安装、注册、绑定、heartbeat、状态展示形成稳定闭环；
3. 明确 `nodemanage` 只承担控制面职责，不侵入执行面、查询引擎和链路元数据引擎；
4. 为后续 API 契约、数据模型设计、安装协议和实现结构提供上层约束。

## 组件定位与边界

`nodemanage` 的定位保持和概述文档一致：

> `nodemanage` 是节点管理体系的控制平面，负责节点纳管、安装编排、节点元信息管理和跨组件协同。

把这个定位展开后，`nodemanage` 在第一阶段应该负责以下几类职责。

### 1. 节点主体建模

`nodemanage` 负责定义“什么是一个被平台识别的节点”。

它管理：

- 节点身份；
- 访问地址和环境信息；
- 节点来源；
- 节点标签和分组信息；
- 节点纳管生命周期。

### 2. 节点纳管流程编排

`nodemanage` 负责把“录入节点、选择安装制品、执行安装、等待注册、进入已纳管状态”串成一条可追踪的控制面流程。

这里的重点是“编排和记录”，不是“承包一切节点侧执行能力”。

### 3. 节点与 agent 的绑定关系管理

`nodemanage` 负责知道：

- 这个节点当前是否已经注册 agent；
- 这个 agent 是否和当前节点稳定绑定；
- 某次重装或替换 agent 后，绑定关系怎样变更。

### 4. 数据链路引用管理

`nodemanage` 不拥有 heartbeat 数据链路的定义，但需要保存自己依赖的链路引用，例如：

- `data_link_id`
- `link_purpose = node_heartbeat`
- `owner_service = nodemanage`

也就是说，`nodemanage` 保存的是“我依赖哪条链路”，而不是“这条链路内部到底怎么存、怎么查”。

### 5. 状态聚合与平台 API

`nodemanage` 对外暴露的是节点管理语义，例如：

- 创建节点；
- 发起纳管；
- 查询节点详情；
- 查询安装状态；
- 查询在线状态。

这些 API 面向平台和前端，而不是面向底层指标系统。

## 明确不负责的事情

为了守住边界，`nodemanage` 在第一阶段明确不负责以下内容：

1. 不直接实现底层指标查询逻辑；
2. 不直接实现底层存储查询和模板拼接逻辑；
3. 不作为复杂批量任务调度器；
4. 不拥有 heartbeat 链路定义、`result_table` 映射和查询模板；
5. 不直接承载 `job-manage` 的远程任务执行域；
6. 不把 `rsagent` 的执行协议细节塞进节点主对象模型。

特别强调：

- **执行面** 仍然在 `rsagent` 和后续任务组件；
- **运行态查询** 仍然在 `query-engine`；
- **节点状态解释** 在 `nodemanage`；
- **链路元数据真相** 仍然在 `datalink-engine`。

## 与其他组件的边界

### 与 rsagent 的边界

`rsagent` 是节点侧代理，负责：

- 注册；
- heartbeat 上报；
- 配置同步；
- 节点侧执行能力。

`nodemanage` 不替代 `rsagent` 的职责，只负责：

- 决定节点是否应该被纳管；
- 决定纳管时应下发哪些安装参数和链路引用；
- 接收注册结果并记录绑定关系。

### 与 datalink-engine 的边界

`datalink-engine` 负责：

- 创建和管理数据链路；
- 维护 `datasource`、`etl_pipeline`、`result_table`；
- 持有链路元数据定义真相。

`nodemanage` 只负责：

- 在启动或初始化时申请/对齐自己需要的 heartbeat 链路；
- 保存 `data_link_id` 及其业务用途引用；
- 在安装配置里把 `data_link_id` 下发给 `rsagent`。

#### nm 启动时创建 heartbeat datalink 的组织方式

第一阶段建议把 heartbeat 链路明确建模为：

- **一条由 `nodemanage` 初始化时创建或对齐的共享 datalink**；
- 所有被纳管节点都共用这条 datalink；
- 节点差异不通过“每个 node 一条 datalink”表达；
- 节点差异通过 heartbeat 数据里的维度表达。

也就是说，`nodemanage` 在启动时调用 `ApplyDataLink`，组织的是一份**共享链路定义**，而不是按节点逐个创建链路。

这条共享 heartbeat datalink 的职责是：

- 定义“谁在产生日志/指标”；
- 定义“写到哪种存储”；
- 定义“查询时按哪些维度过滤和归并”；
- 让后续 `rsagent` 上报和 `query-engine` 查询都围绕同一个 `data_link_id` 展开。

#### 共享 heartbeat datalink 的参数组织

从当前仓库 `datalink-engine` 的 `ApplyDataLinkSpec` 看，`nodemanage` 初始化时至少需要组织以下几层参数：

1. `DataLink` 主体
2. `datasource`
3. `etl_pipeline`
4. `result_table`

但对 `nodemanage` 来说，真正需要在设计里明确的重点不是“字段列表抄一遍”，而是哪些是**共享静态定义**，哪些是**节点运行时维度**。

##### 1. 共享静态定义参数

以下参数建议由 `nodemanage` 在启动时统一组织，并写入一条共享 heartbeat datalink：

- `name`
- `description`
- `domain = nodemanage`
- `owner_service = nodemanage`
- `data_type = metric`
- `status = active`
- `datasource.producer = rsagent`
- `datasource.collect_method`
- `datasource.protocol`
- `datasource.interval_seconds`
- `datasource.dimension_keys`
- `etl_pipeline.mode`
- `etl_pipeline.config`
- `result_table.storage_type`
- `result_table.result_table_name`
- `result_table.metric_name`
- `result_table.query_template`
- `result_table.retention_days`

其中建议特别固定以下几项：

- `domain = nodemanage`
- `owner_service = nodemanage`
- `data_type = metric`
- `datasource.producer = rsagent`

这样在语义上就比较清楚：

- 链路归属是 `nodemanage`；
- 实际数据生产者是 `rsagent`；
- 数据类型是 heartbeat metric。

##### 2. 作为 heartbeat 维度的节点字段

以下字段不应作为“每个节点单独创建链路时的参数”，而应作为共享 heartbeat 数据里的维度：

- `node_id`
- `agent_id`
- `node_ip`

也就是说：

- 它们应进入 `datasource.dimension_keys`；
- 也应成为 `rsagent` 每次 heartbeat 上报时携带的维度；
- `query-engine` 和上层服务基于这些维度查询和组织规则。

这几个字段的职责分工建议是：

- `node_id`：平台内稳定节点主体标识；
- `agent_id`：节点侧 agent 实例标识；
- `node_ip`：节点侧访问地址或观测维度补充。

#### 为什么不按节点创建 datalink

第一阶段不建议采用“每个 node 一条 heartbeat datalink”的方式，原因是：

1. 链路数量会随着节点数膨胀；
2. 初始化和重试逻辑会更复杂；
3. `query-engine` 查询模型会被迫围绕“多链路”展开；
4. 实际上节点差异已经可以通过维度表达，没有必要再提升为链路级对象。

因此更合适的方式是：

- `nodemanage` 只创建一条共享 heartbeat datalink；
- 每个 `rsagent` 往这条链路写自己的 heartbeat；
- 节点差异通过 `node_id / agent_id / node_ip` 等维度表达。

#### 第一阶段建议的 heartbeat datalink 组织思路

如果按当前文档约束继续收敛，第一阶段建议把 `nodemanage` 初始化时提交给 `ApplyDataLink` 的 heartbeat 链路定义组织成：

- `name = nodemanage_node_heartbeat`
- `domain = nodemanage`
- `owner_service = nodemanage`
- `data_type = metric`
- `status = active`
- `datasource.producer = rsagent`
- `datasource.dimension_keys = ["node_id", "agent_id", "node_ip"]`
- `etl_pipeline.mode = passthrough`
- `result_table.storage_type = victoriametrics`
- `result_table.result_table_name = nm_node_heartbeat`
- `result_table.metric_name = nm_node_heartbeat`

这里的关键点是：

- `dimension_keys` 负责描述节点侧差异；
- `result_table_name` / `metric_name` 负责描述共享链路落点；
- `query_template` 负责后续查询入口；
- `interval_seconds` / `retention_days` 等参数则决定采集节奏和保留策略。

#### `ApplyDataLink` heartbeat spec 示例

如果把上面的设计进一步压实成一次 `nodemanage` 初始化时发给 `datalink-engine` 的请求体草稿，第一阶段建议接近如下结构：

```json
{
  "name": "nodemanage_node_heartbeat",
  "description": "shared heartbeat datalink for all managed nodes",
  "domain": "nodemanage",
  "owner_service": "nodemanage",
  "data_type": "metric",
  "status": "active",
  "status_message": null,
  "datasource": {
    "producer": "rsagent",
    "data_type": "metric",
    "collect_method": "agent",
    "protocol": "http",
    "interval_seconds": 60,
    "labels": {
      "domain": "nodemanage",
      "link_purpose": "node_heartbeat"
    },
    "dimension_keys": ["node_id", "agent_id", "node_ip"],
    "auth_ref": null,
    "config": {}
  },
  "etl_pipeline": {
    "mode": "passthrough",
    "config": {}
  },
  "result_table": {
    "result_table_name": "nm_node_heartbeat",
    "storage_type": "victoriametrics",
    "storage_cluster": "default",
    "database": null,
    "table_name": null,
    "metric_name": "nm_node_heartbeat",
    "query_template": "query metric_name=nm_node_heartbeat by node_id,agent_id,node_ip within time_range",
    "schema": {
      "timestamp": "datetime",
      "node_id": "string",
      "agent_id": "string",
      "node_ip": "string"
    },
    "retention_days": 7
  }
}
```

这个示例的作用不是冻结最终 JSON 文案，而是明确：

- `nodemanage` 在启动时到底要准备哪些参数；
- 哪些参数描述共享 heartbeat 链路；
- 哪些字段只是运行时维度，而不是链路主体本身。

#### 示例里的关键参数说明

为了避免把示例看成“纯样例文本”，这里把最关键的几个字段再解释一下：

- `name`
  - 表示这条共享 heartbeat datalink 的稳定逻辑名称；
  - 也是 `datalink-engine` 做逻辑幂等匹配的重要字段之一。

- `domain = nodemanage`
  - 表示这条链路属于 `nodemanage` 业务域；
  - 便于后续按 domain 检索和治理。

- `owner_service = nodemanage`
  - 表示链路定义的拥有者是 `nodemanage`；
  - 与实际写数据的 `producer = rsagent` 要区分开。

- `datasource.producer = rsagent`
  - 表示数据由节点侧 `rsagent` 产生；
  - 这不改变链路归属，链路归属仍然在 `nodemanage`。

- `datasource.dimension_keys = ["node_id", "agent_id", "node_ip"]`
  - 表示查询和归并时依赖的维度；
  - 这些字段不是“建三条链路”，而是“一条链路里的三类维度”。

- `etl_pipeline.mode = passthrough`
  - 第一阶段优先走直通模式；
  - 避免一开始就引入复杂转换逻辑。

- `result_table.storage_type = victoriametrics`
  - 表示 heartbeat 第一阶段落到 VictoriaMetrics；
  - 与当前整体设计方向保持一致。

- `result_table.metric_name = nm_node_heartbeat`
  - 表示在指标后端里的统一 metric 名；
  - 后续 `query-engine` 会围绕这个 metric 和维度来做查询。

- `result_table.query_template`
  - 这里是“查询入口模板”的占位表达；
  - 第一阶段建议采用后端原生模板，不再额外设计中间 DSL；
  - `query-engine` 负责读取模板并注入约定变量。

#### heartbeat `query_template` 约定

第一阶段建议把 heartbeat 的 `query_template` 明确收敛成：

- **面向具体存储后端的原生查询模板**；
- 模板中允许出现少量固定占位变量；
- `query-engine` 负责读取模板、注入变量、执行实际查询；
- 上层服务不直接拼接后端原生查询语句。

这样做的原因是：

1. 当前仓库里的 `query_template` 本身就是 `string`，没有中间 DSL 模型；
2. 第一阶段主要落点是 VictoriaMetrics，没有必要再抽一层语义模板语言；
3. 把复杂度控制在“固定变量替换”会更容易让 `nm`、`query-engine` 和 `datalink-engine` 文档先对齐。

#### 第一阶段建议固定的 heartbeat 模板变量

heartbeat 场景下，第一阶段建议只允许以下占位变量：

- `$metric_name`
- `$node_id`
- `$agent_id`
- `$node_ip`
- `$start_at`
- `$end_at`

这些变量的职责建议是：

- `$metric_name`：定位共享 heartbeat metric；
- `$node_id`：按平台节点主体查询；
- `$agent_id`：按 agent 实例补充过滤；
- `$node_ip`：按节点地址补充过滤；
- `$start_at` / `$end_at`：表达时间窗口。

第一阶段不建议再开放任意变量名，否则模板治理和查询安全边界会过早变复杂。

#### 第一阶段 heartbeat 查询类型约定

第一阶段建议只围绕以下三类 heartbeat 查询组织模板和查询逻辑：

1. 单节点最近 heartbeat 查询；
2. 批量节点最近 heartbeat 查询；
3. 时间范围 heartbeat 查询。

也就是说：

- `query_template` 应围绕 heartbeat metric + 维度过滤 + 时间窗口来表达；
- 不应该在第一阶段承载复杂聚合、报表或通用查询构造能力。

#### heartbeat `query_template` 的文档级约束

为了让后续 `query-engine` 实现保持稳定，第一阶段建议在文档层先冻结以下约束：

- 模板是**后端原生模板**，不是平台通用 DSL；
- 模板变量采用 `$variable` 形式；
- 允许的过滤维度只包括 `node_id`、`agent_id`、`node_ip`；
- 时间范围统一通过 `$start_at` / `$end_at` 表达；
- `query-engine` 负责变量注入和执行；
- `nodemanage` 只负责在初始化 heartbeat datalink 时把模板定义好。

#### heartbeat `query_template` 示例表达

第一阶段文档里不必强行冻结某条精确的 PromQL，但建议把它理解成接近如下语义：

```text
query $metric_name
where node_id = $node_id
  and agent_id = $agent_id
  and node_ip = $node_ip
  and timestamp between $start_at and $end_at
```

如果实际后端是 VictoriaMetrics，那么最终落地可以是对应的后端原生表达；但在 `nodemanage` 设计文档里，更重要的是先把：

- metric 名来源；
- 允许的维度；
- 时间窗口变量；
- 模板归谁定义、谁执行

这些边界写清楚。

#### VictoriaMetrics 风格的 heartbeat 模板示例

如果第一阶段 heartbeat 数据实际落在 VictoriaMetrics，那么 `query_template` 可以进一步理解成接近如下风格的后端原生模板：

```text
max_over_time($metric_name{node_id="$node_id",agent_id="$agent_id",node_ip="$node_ip"}[$window])
```

或者在时间范围查询场景中，接近如下风格：

```text
$metric_name{node_id="$node_id",agent_id="$agent_id",node_ip="$node_ip"}
```

再由 `query-engine` 在执行时补充：

- 实际查询时间范围（`$start_at` / `$end_at`）；
- 是否是 instant query 还是 range query；
- 哪些维度是必填，哪些维度允许为空。

这里建议第一阶段把文档重点放在“模板变量和查询职责”上，而不是把某一条具体 PromQL 当作唯一标准答案。

#### heartbeat 模板执行规则建议

如果采用后端原生模板，第一阶段建议再补充以下执行规则：

1. `metric_name` 必填；
2. `node_id` 应作为主过滤维度；
3. `agent_id` / `node_ip` 可以作为补充过滤维度；
4. 最近 heartbeat 查询优先采用“近窗口聚合”表达；
5. 时间范围 heartbeat 查询优先采用 range query；
6. 缺少可选维度时，`query-engine` 应允许省略对应过滤条件，而不是强制拼空值过滤。

也就是说：

- 模板里允许定义完整维度集合；
- 但执行层可以根据调用参数裁剪实际过滤条件；
- 第一阶段不要求模板作者为每种查询类型写完全不同的模板语言。

#### 哪些参数应从配置读取，哪些参数应在代码里固定

第一阶段建议再把这份 spec 拆成两类来源：

##### 1. 建议固定在代码或默认常量里的参数

- `domain = nodemanage`
- `owner_service = nodemanage`
- `data_type = metric`
- `datasource.producer = rsagent`
- `datasource.dimension_keys = ["node_id", "agent_id", "node_ip"]`
- `etl_pipeline.mode = passthrough`

这些是体系设计级约束，第一阶段不建议让部署方随意改动。

##### 2. 建议从配置读取或允许覆盖的参数

- `datasource.protocol`
- `datasource.interval_seconds`
- `result_table.storage_cluster`
- `result_table.metric_name`
- `result_table.result_table_name`
- `result_table.query_template`
- `result_table.retention_days`

这些参数更接近部署环境、存储环境或保留策略，适合作为配置面存在。

#### 与 query-engine 的协作含义

采用共享 heartbeat datalink 后，`query-engine` 的查询模型也会更稳定：

- 先通过固定 `data_link_id` 找到共享 heartbeat 链路；
- 再按 `node_id`、`agent_id`、`node_ip` 等维度查询具体节点数据；
- 上层服务再基于返回结果组织自己的在线/离线或其他规则。

这也再次说明：

- `nodemanage` 初始化时真正要解决的是“先把共享 heartbeat 链路定义好”；
- 不是为每个节点生成独立的 datalink。

#### nm 初始化时创建 heartbeat datalink 的时序与幂等逻辑

第一阶段建议把 `nodemanage` 启动期的 heartbeat datalink 初始化流程收敛成一条固定时序：

1. `nodemanage` 启动；
2. 加载本地配置，得到 heartbeat datalink 的静态定义参数；
3. 组装 `ApplyDataLinkSpec`；
4. 调用 `datalink-engine` 的 `ApplyDataLink`；
5. 获取返回的 `DataLinkBundle`；
6. 保存其中的 `data_link_id` 与 `result_table_name` 引用；
7. 后续节点安装和 `query-engine` 查询统一使用这条共享 heartbeat 链路。

也就是说：

- `ApplyDataLink` 是 `nodemanage` 初始化期的基础步骤；
- 它不是每次创建节点时都重新做一次；
- 它也不是每次 `rsagent` heartbeat 上报时参与的流程。

#### 为什么初始化时就做 `ApplyDataLink`

第一阶段建议在 `nodemanage` 启动时就先对齐 heartbeat datalink，原因是：

1. 节点安装时需要尽早拿到稳定的 `data_link_id`；
2. 如果把链路创建延后到“第一次纳管节点时”再做，会把安装路径和链路初始化路径耦合在一起；
3. 初始化阶段先对齐链路，有利于把失败暴露得更早、更集中；
4. 共享 heartbeat datalink 本来就是服务级资源，不是节点级资源。

因此更合理的理解是：

- `nodemanage` 先准备好一条公共 heartbeat 基础设施；
- 后续所有节点纳管都复用它。

#### `ApplyDataLink` 的幂等语义如何被 `nm` 使用

从当前 `datalink-engine` 的实现看，`ApplyDataLink` 的逻辑幂等主要依赖：

- `domain`
- `owner_service`
- `name`

同时 `result_table_name` 还需要保持唯一。

这对 `nodemanage` 的设计含义是：

- `name` 必须稳定，不能每次启动随机生成；
- `domain = nodemanage` 和 `owner_service = nodemanage` 也必须稳定；
- `result_table.result_table_name` 必须稳定，不能把时间戳或实例号拼进去；
- 这样 `nodemanage` 重启后再次执行 `ApplyDataLink`，才能复用同一条逻辑链路，而不是创建出新链路。

所以第一阶段建议把以下字段视为**幂等主标识的一部分**：

- `name = nodemanage_node_heartbeat`
- `domain = nodemanage`
- `owner_service = nodemanage`
- `result_table.result_table_name = nm_node_heartbeat`

#### `idempotency_key` 的使用建议

除了逻辑幂等匹配外，`datalink-engine` 当前还支持 `ApplyDataLinkOptions.idempotency_key`。

对 `nodemanage` 来说，第一阶段建议采用以下策略：

- 可以传 `idempotency_key`，但不要依赖它代替逻辑命名稳定性；
- 逻辑命名稳定性仍然是主策略；
- `idempotency_key` 更适合用来吸收一次初始化过程中的重复请求或网络重试。

也就是说：

- `name/domain/owner_service/result_table_name` 负责“这是不是同一条链路”；
- `idempotency_key` 负责“这一轮请求重放要不要重复执行”。

第一阶段如果需要一个建议值，可以采用：

- `idempotency_key = nodemanage-bootstrap-heartbeat-datalink`

但这只是初始化请求幂等键建议值，不影响链路的长期逻辑身份。

#### 初始化成功后 `nm` 应该保存什么

`ApplyDataLink` 成功后，第一阶段建议 `nodemanage` 至少保存以下引用信息：

- `data_link_id`
- `result_table_name`
- `link_purpose = node_heartbeat`
- `owner_service = nodemanage`

保存这些引用的目的不是复制一份链路真相，而是让 `nodemanage` 后续能够：

- 在安装配置里把 `data_link_id` 下发给 `rsagent`；
- 在查询时把 `data_link_id` 提供给 `query-engine`；
- 在需要展示或排障时，知道自己当前绑定的是哪条 heartbeat 链路。

#### `nm` 重启后的行为建议

`nodemanage` 重启后，不应假设本地缓存里的 `data_link_id` 永远可信。

第一阶段更稳妥的策略是：

- 启动后重新执行一次 `ApplyDataLink`；
- 利用 `datalink-engine` 的幂等语义拿回当前链路；
- 再刷新本地保存的 `data_link_id` / `result_table_name` 引用。

这样可以避免：

- 本地状态丢失后无法恢复；
- 运维手工修复链路后，本地引用过旧；
- 多实例部署时仅靠本地缓存造成状态漂移。

#### 初始化失败时的处理建议

第一阶段建议把 `ApplyDataLink` 失败分成两类：

##### 1. 配置或参数错误

例如：

- `result_table_name` 冲突；
- `etl_pipeline` 不合法；
- 存储类型与字段组合不兼容。

这类问题建议：

- 直接让 `nodemanage` 启动失败或进入明显的 degraded 状态；
- 不要静默跳过；
- 因为这说明 heartbeat 基础设施本身没有准备好。

##### 2. 暂时性依赖错误

例如：

- `datalink-engine` 暂时不可达；
- 网络抖动；
- 请求超时。

这类问题建议：

- 允许有限次重试；
- 重试时继续使用相同逻辑标识和幂等键；
- 如果多次失败，`nodemanage` 应明确暴露“heartbeat datalink 未就绪”的服务状态。

#### 第一阶段的失败恢复策略

第一阶段建议采用简单而明确的恢复策略：

1. 启动时先尝试 `ApplyDataLink`；
2. 如果是暂时性错误，则按固定间隔重试；
3. 如果是配置错误，则不自动无限重试；
4. 只有在 heartbeat datalink 就绪后，才认为 `nodemanage` 的纳管基础设施准备完成。

这也意味着：

- `nodemanage` 可以启动进程成功；
- 但如果 datalink 初始化失败，服务状态不应被视为 fully ready。

#### 与节点纳管流程的关系

把 heartbeat datalink 初始化放在服务启动期后，节点纳管流程就可以简化成：

1. `nodemanage` 已拥有稳定 heartbeat `data_link_id`；
2. 用户发起节点纳管；
3. `nodemanage` 在安装配置中把该 `data_link_id` 下发给节点；
4. `rsagent` 启动并开始往共享 heartbeat datalink 写数据。

这样职责更清楚：

- 服务初始化阶段解决“链路是否存在”；
- 节点纳管阶段解决“节点如何使用这条链路”。

### 与 query-engine 的边界

`query-engine` 负责：

- 根据 `data_link_id` 获取链路元数据；
- 查询底层存储；
- 返回统一查询结果。

`nodemanage` 负责周期性消费 `query-engine` 的结果，按 `node_id` 维度与规则计算节点状态，并将其与本地管理态拼装后返回。

第一阶段里，`query-engine` 对 `nodemanage` 提供的运行态结果，建议仅限于：

- 最近一次 heartbeat 记录；
- heartbeat 查询结果；
- 时间范围内的运行事实数据。

在线/离线、健康摘要等具体业务解释规则由 `nodemanage` 自己决定；更宽的查询能力，本文统一标记为 **TBD**。

#### nm 周期性 heartbeat 状态计算主线

第一阶段建议把 `nodemanage` 的节点状态计算主线明确成：

1. 定时任务触发 heartbeat 状态刷新；
2. `nodemanage` 使用固定 heartbeat `data_link_id` 调用 `query-engine`；
3. `query-engine` 返回 heartbeat 链路上的时序查询结果；
4. `nodemanage` 按 `node_id` 维度归并结果；
5. `nodemanage` 按自身规则计算 `node_id -> status`；
6. 计算结果进入 `nodemanage` 的节点状态视图或聚合结果。

这条主线里：

- `query-engine` 只提供 heartbeat 数据；
- `nodemanage` 才是节点状态计算者；
- 后续前端和其他服务应优先消费 `nodemanage` 的节点状态结果。

### 与 job-manage 的边界

`job-manage` 负责基于节点做远程任务执行与任务调度。

`nodemanage` 给 `job-manage` 提供的是：

- 可被选择的节点主体；
- 节点标签/分组；
- 节点基础状态；
- 节点是否处于可执行任务的管理态。

`nodemanage` 不应直接承接“脚本执行平台”这一定位，否则边界会和 `job-manage` 混淆。

## 核心领域对象

### 1. Node

`Node` 是 `nodemanage` 的主对象，表示平台内一个被识别、可纳管、可绑定 agent、可被查询状态的机器节点。

建议至少包含以下信息：

- `node_id`：平台内唯一标识；
- `node_name`：节点名称；
- `access_host` / `ip`：访问地址；
- `environment`：环境，例如 dev/test/prod；
- `labels`：节点标签；
- `source_type`：节点来源，例如 manual/import；
- `lifecycle_state`：节点生命周期状态；
- `install_status`：安装状态；
- `register_status`：注册状态；
- `last_known_agent_id`：最近一次成功绑定的 agent 标识；
- `created_at` / `updated_at`。

### Node 的状态建议

- `draft`：节点已录入，但未发起纳管；
- `onboarding`：正在纳管中；
- `managed`：已稳定纳管；
- `disabled`：节点被手动禁用；
- `deleted`：软删除。

### 2. NodeInstallTask

`NodeInstallTask` 用于记录一次节点纳管任务。

它的价值不是“让节点真正执行任意脚本”，而是确保安装过程具备：

- 可追踪；
- 可回放；
- 可排障；
- 可审计。

建议包含：

- `install_task_id`；
- `node_id`；
- `executor_type`：执行方式，例如 ssh / worker；
- `initiator`：发起人；
- `task_state`；
- `current_step`；
- `error_code` / `error_message`；
- `started_at` / `finished_at`。

### NodeInstallTask 状态建议

- `pending`
- `running`
- `waiting_register`
- `succeeded`
- `failed`
- `cancelled`

这里建议在全文中统一使用“安装任务”作为主名，不再额外并列“纳管任务”“注册任务”等多种名称。

### 3. NodeAgentBinding

`NodeAgentBinding` 表达“节点主体”和“节点上的 agent 实例”之间的关系。

建议至少包含：

- `node_id`；
- `agent_id`；
- `binding_state`；
- `first_registered_at`；
- `last_handshake_at`；
- `unbind_reason`。

### 绑定状态建议

- `bound`：当前稳定绑定；
- `stale`：绑定关系存在但已不新鲜，例如心跳长时间未刷新；
- `unbound`：已解除绑定。

### 4. NodeDataLinkRef

`NodeDataLinkRef` 不是链路定义本身，而是 `nodemanage` 依赖的数据链路引用。

第一阶段至少包含 heartbeat 链路引用：

- `domain = nodemanage`
- `link_purpose = node_heartbeat`
- `data_link_id`
- `owner_service = nodemanage`

必要时也可以补充：

- `result_table_name`（仅用于管理辅助定位，不作为跨组件稳定引用主键）；
- `enabled`；
- `updated_at`。

### 5. NodeStatusSnapshot

`NodeStatusSnapshot` 是一个读模型，用于表达节点当前的聚合状态。

它不是链路真相源，也不是指标存储真相源，而是 `nodemanage` 返回给上层时的状态摘要。

建议包含：

- `node_id`；
- `online_status`：`online` / `offline` / `unknown`；
- `last_heartbeat_at`；
- `status_reason`；
- `aggregated_at`。

这里必须明确一个约束：

- `unknown` 和 `offline` 不能混用；
- 当 `query-engine` 不可达或链路不可查询时，应优先返回 `unknown`，而不是误判为 `offline`。

## 安装制品域设计

这一部分是本文新补充的详细设计方向，但目前仓库文档尚未完整定义其实现方式。因此以下内容按“可接受的第一阶段默认设计”组织，其中未被现有文档证明的部分，统一视为 **Assumption**。

### 设计动机

如果 `nodemanage` 只记录“发起了一次安装”，却不知道：

- 当时选了哪个安装包；
- 用了哪个脚本；
- 针对什么 OS/arch 做了选择；
- 安装时最终渲染了哪些参数；

那么安装链路就无法做到稳定审计和排障。

因此，`nodemanage` 在第一阶段至少需要把“安装制品选择结果”纳入控制面记录中。

### Assumption：InstallPackage

`InstallPackage` 表示一个可被安装流程引用的安装包元数据对象。

建议包含：

- `package_id`
- `name`
- `package_type`：例如 agent / plugin
- `version`
- `os_family`
- `os_distribution`
- `arch`
- `package_url`
- `checksum`
- `status`

说明：

- 现有仓库已经有 `rsagent_package_url` 这一配置面，但没有完整包元数据建模；
- 因此这里的对象定义属于在现有能力上的合理扩展，而不是现有实现事实。

### Assumption：InstallScript

`InstallScript` 表示一次安装流程中引用的脚本元数据对象。

建议包含：

- `script_id`
- `name`
- `script_kind`：bootstrap / install / upgrade / uninstall / register
- `version`
- `os_family`
- `os_distribution`
- `arch`
- `interpreter`
- `script_source`
- `content_ref`
- `status`

这里要特别守住边界：

- `InstallScript` 的存在是为了描述安装流程所使用的脚本制品；
- 它不意味着 `nodemanage` 会演进成通用脚本执行平台；
- 任何与任务执行、批量执行相关的领域，仍应留在 `job-manage` 或后续专门组件中。

### Assumption：InstallArtifactProfile

为了避免和 datalink 文档中的 `profile` 概念混淆，本文建议在详细设计中优先使用 `InstallArtifactProfile` 或 `install_profile`。

它表示一组安装制品选择规则，目标是回答：

- 针对某种 OS / distro / arch，默认选哪个 agent 包；
- 默认选哪个安装脚本；
- 默认带哪些插件包。

建议包含：

- `install_profile_id`
- `name`
- `target_os_family`
- `target_os_distribution`
- `target_arch`
- `default_agent_package_id`
- `default_install_script_id`
- `default_plugin_package_ids`
- `status`

### Assumption：InstallResolution

`InstallResolution` 表示一次安装任务最终解析出的制品选择结果。

它的意义在于：

- 历史安装任务必须能回看当时到底用了什么；
- 即使之后默认 profile 改了，历史任务记录也不应该被“追改”；
- 故障排查时需要精确知道这次安装最终落了哪组制品。

建议包含：

- `resolution_id`
- `install_task_id`
- `resolved_install_profile_id`
- `resolved_agent_package_id`
- `resolved_install_script_id`
- `resolved_plugin_package_ids`
- `resolved_os_family`
- `resolved_os_distribution`
- `resolved_arch`

## 安装任务与制品解析关系

在引入安装制品域之后，`NodeInstallTask` 建议补充以下维度：

- `target_os_family`
- `target_os_distribution`
- `target_arch`
- `resolved_install_profile_id`
- `resolved_agent_package_id`
- `resolved_install_script_id`
- `resolved_plugin_package_ids`

这部分字段的目标不是让 `NodeInstallTask` 取代 `InstallResolution`，而是让安装任务在审计和查询时具备足够的关键信息。

## 节点 OS / 架构识别策略

### Assumption

本文默认采用“混合模式”：

1. 节点创建时允许先录入访问地址和基础信息；
2. 如果用户手工提供了 OS / arch 信息，先作为候选值使用；
3. 安装前由 `nodemanage` 通过 SSH 或等价方式探测真实 OS / distro / arch；
4. 探测结果优先于手工录入值，用于制品解析。

这样设计的原因是：

- 完全手工录入容易导致安装包选择错误；
- 完全依赖预先探测又会增加初次录入复杂度；
- 混合模式更符合第一阶段“先打通主链路”的目标。

## 凭据引用设计

### Assumption

`nodemanage` 第一阶段建议支持“凭据引用为主，任务级临时覆盖为辅”的模式。

也就是说：

- 节点或节点组可以绑定一个 `credential_ref`；
- 安装任务允许在特殊场景下临时覆盖；
- `nodemanage` 只管理凭据引用，不在文档里假定保存明文凭据。

这一块在现有仓库文档中没有成型设计，因此当前只作为默认方向，不视为既成事实。

## 第一阶段主流程

### 1. 节点纳管主链路

建议把第一阶段主链路细化为以下步骤：

1. 用户创建待纳管节点；
2. `nodemanage` 保存节点主对象；
3. 节点发起纳管请求；
4. `nodemanage` 创建安装任务；
5. `nodemanage` 获取或探测目标节点的 OS / distro / arch；
6. `nodemanage` 解析安装制品选择结果；
7. `nodemanage` 向 `datalink-engine` 对齐 heartbeat 链路，并获取 `data_link_id`；
8. `nodemanage` 生成安装配置和安装参数；
9. 执行器在目标节点上安装 `rsagent`；
10. `rsagent` 启动后向 `nodemanage` 注册；
11. `nodemanage` 建立节点和 agent 的绑定关系；
12. `nodemanage` 进入已纳管状态；
13. `query-engine` 根据 `data_link_id` 查询 heartbeat 数据；
14. `nodemanage` 周期性计算节点状态，并聚合管理态和运行态后返回给前端。

### 2. 运行态查询主链路

状态查询主链路建议如下：

1. 前端或平台服务请求节点状态；
2. `nodemanage` 查询本地节点主对象、安装状态、注册状态、绑定关系；
3. `nodemanage` 读取 `NodeDataLinkRef` 中的 heartbeat 链路引用；
4. `nodemanage` 调用 `query-engine` 查询节点最近 heartbeat；
5. `query-engine` 返回统一 heartbeat 查询结果；
6. `nodemanage` 按 `node_id` 维度计算节点状态，并组合为统一的节点状态响应。

### 3. 注册绑定主链路

注册绑定建议遵循以下原则：

1. `rsagent` 注册成功后，`nodemanage` 才能把安装任务推进到成功态；
2. 安装成功但未注册，不应直接视为已纳管；
3. `agent_id` 与 `node_id` 绑定冲突时，不应静默覆盖；
4. 发生冲突时应返回明确错误，并进入人工确认或显式 rebind 流程。

## API 设计建议

本文不冻结最终协议字段，但建议第一阶段接口围绕以下语义分组展开。

### 1. 节点主数据接口

- `POST /api/nm/v1/nodes`
- `GET /api/nm/v1/nodes`
- `GET /api/nm/v1/nodes/:node_id`
- `PATCH /api/nm/v1/nodes/:node_id`
- `DELETE /api/nm/v1/nodes/:node_id`

### 2. 节点纳管与安装接口

- `POST /api/nm/v1/nodes/:node_id/install`
- `GET /api/nm/v1/install-tasks/:install_task_id`
- `POST /api/nm/v1/install-tasks/:install_task_id/cancel`

### 3. agent 注册接口

- `POST /api/nm/v1/agents/register`

### 4. 节点状态接口

- `GET /api/nm/v1/nodes/:node_id/status`
- `GET /api/nm/v1/nodes/status:batch`

### 5. 绑定关系接口

- `GET /api/nm/v1/nodes/:node_id/binding`
- `POST /api/nm/v1/nodes/:node_id/rebind`

### 6. 数据链路引用接口

- `GET /api/nm/v1/datalinks`
- `POST /api/nm/v1/datalinks:reconcile`

### 7. 安装制品接口（Assumption）

以下接口是基于当前详细设计方向建议补充的安装制品管理面，现阶段应视为默认设计而非已实现事实：

- `POST /api/nm/v1/install-packages`
- `GET /api/nm/v1/install-packages`
- `POST /api/nm/v1/install-scripts`
- `GET /api/nm/v1/install-scripts`
- `POST /api/nm/v1/install-profiles`
- `GET /api/nm/v1/install-profiles`
- `POST /api/nm/v1/nodes/:node_id/install:resolve`

其中 `install:resolve` 的目标是让系统在真正安装前就能展示：

- 当前节点会命中哪个安装 profile；
- 最终会选择哪个 agent 包；
- 会使用哪个安装脚本；
- 将附带哪些插件包。

## 状态机建议

### 1. Node 生命周期

- `draft`
- `onboarding`
- `managed`
- `disabled`
- `deleted`

### 2. 安装状态

- `not_started`
- `installing`
- `waiting_register`
- `installed`
- `install_failed`

### 3. 注册状态

- `unregistered`
- `registering`
- `registered`
- `expired`

### 4. 绑定状态

- `bound`
- `stale`
- `unbound`

这些状态名只是当前详细设计中的建议集合；最终命名可在后续 API 契约和数据模型文档中统一收敛。

## 错误与失败回滚建议

### 1. 安装失败

当安装执行失败时：

- `NodeInstallTask` 应进入 `failed`；
- 节点的安装状态进入失败态；
- 节点主体不应被自动删除；
- 应保留制品解析结果、错误码、错误信息和失败步骤。

### 2. 安装完成但注册超时

当安装执行完成，但 `rsagent` 没有在预期时间内注册时：

- 安装任务应进入 `waiting_register` 或最终失败态；
- 该节点不能直接转为 `managed`；
- 应返回明确错误原因，例如注册超时。

### 3. 绑定冲突

当一个 `agent_id` 已经绑定其他活动节点，或当前节点收到不符合预期的 agent 注册时：

- 不应自动覆盖；
- 应明确报错；
- 是否允许显式 rebind，留待后续契约文档确定。

### 4. 状态查询失败

当 `query-engine` 不可用或链路不可查询时：

- `nodemanage` 应优先返回 `unknown`；
- 不应直接把节点标记为 `offline`；
- 应在 `status_reason` 中体现失败来源。

## 第一阶段约束与不展开项

第一阶段建议只锁定以下最小闭环：

1. 录入节点；
2. 发起安装；
3. 安装 `rsagent`；
4. 对齐 heartbeat 链路；
5. 完成注册绑定；
6. 查询并展示在线状态。

以下内容在本阶段不展开，统一视为后续增强项或 TBD：

- 复杂批量纳管编排；
- 通用升级/回滚平台；
- 多链路、多指标、多运行态解释模型；
- 凭据中心或密钥系统集成细节；
- 安装制品签名、供应链校验、完整兼容矩阵；
- 节点侧资源指标、插件生命周期、复杂能力协商协议。

## Rust 落地结构建议

这一节不是强制实现结构，只是为了让后续代码实现更容易和领域对象一一对应。

建议后续实现时可以按如下职责拆分：

- `domain/`
  - `node.rs`
  - `install_task.rs`
  - `binding.rs`
  - `datalink_ref.rs`
  - `install_artifact.rs`
- `service/`
  - `node_service.rs`
  - `install_service.rs`
  - `registration_service.rs`
  - `status_service.rs`
- `repository/`
- `integration/`
  - `datalink_engine_client.rs`
  - `query_engine_client.rs`
  - `install_executor.rs`
- `api/`
- `state/`

这里的目标是保持“领域对象”“集成边界”“API 层”分离，避免把 `nodemanage` 做成一个所有逻辑混在一起的大模块。

## Assumptions / TBD 汇总

以下内容在当前仓库文档中尚未被正式定义，因此在本文中统一视为默认设计方向，而不是现有实现事实：

1. `node = machine` 的模型冻结；
2. 安装制品域对象：`InstallPackage`、`InstallScript`、`InstallArtifactProfile`、`InstallResolution`；
3. OS / distro / arch 的自动探测和校正策略；
4. 凭据引用对象及其和节点/安装任务的关系；
5. `install:resolve` 这一安装预演接口；
6. 绑定冲突的最终 rebind 流程；
7. 安装脚本模板渲染方式和最终执行内容审计策略；
8. 安装包版本选择策略是否完全由 `install_profile` 控制；
9. 插件包是否完整纳入第一阶段制品域。

后续如果继续细化 `nodemanage`，建议继续拆出以下文档：

1. `nodemanage API 契约`
2. `nodemanage 数据模型设计`
3. `nodemanage 安装制品模型设计`
4. `nodemanage 安装与纳管时序设计`
5. `rsagent 注册与配置协议`

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/nodemanage概述.md`
- `doc/datalink-engine概述.md`
- `doc/query-engine概述.md`
- `doc/rsagent概述.md`
- `doc/job-manage概述.md`
- `doc/datalink-engine-api契约.md`

后续如这些上游文档发生变化，本文也应同步校对，避免 `nodemanage` 详细设计与其他组件的概述/契约产生漂移。
