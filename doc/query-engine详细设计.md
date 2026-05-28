# query-engine 详细设计

## 文档目标

这篇文档用于在 `doc/query-engine概述.md` 的基础上，把 `query-engine` 从“组件概述”细化为“可执行的详细设计草案”。

本文重点回答以下问题：

- `query-engine` 第一阶段到底负责什么，不负责什么；
- `query-engine` 与 `datalink-engine`、实际存储后端、`nodemanage`、`job-manage`、`rsagent` 的边界如何稳定；
- 第一阶段对外查询接口应该收敛成什么样；
- `query-engine` 内部最小模块应该如何拆分；
- 第一阶段哪些内容需要冻结，哪些内容仍然是后续扩展项。

本文是第一阶段详细设计文档，不等同于最终 API 路径、数据库 schema 或代码实现的冻结版本。凡是当前仓库文档尚未证明的内容，本文会明确标注为 **Assumption** 或 **TBD**。

## 组件定位

`query-engine` 在节点体系中的定位，第一阶段应明确收敛为：

> `query-engine` 是节点体系中的查询接入服务，负责基于 `data_link_id` 解析链路元数据、对接实际存储后端执行 heartbeat 查询，并向上层返回统一查询结果。

这里特别强调：

- `query-engine` 负责“查什么、去哪查、怎么把结果标准化返回”；
- `query-engine` 不负责这些查询结果在业务上意味着什么；
- `query-engine` 不负责业务解释，例如在线/离线判断、复杂健康评分或作业调度结论。

也就是说，第一阶段的 `query-engine` 是：

- 数据链路驱动的查询服务；
- 实际存储后端的接入层；
- heartbeat 查询结果的统一返回层。

它不是：

- 节点控制面；
- 节点在线/离线判断服务；
- 作业可执行性判断服务；
- 数据链路元数据真相服务；
- 通用 BI / 报表平台。

## 设计目标

`query-engine` 第一阶段不追求一开始就变成一个通用查询平台，而是优先把“**通过 `data_link_id` 查询 heartbeat 数据并把结果稳定返回给上层**”这条主链路做扎实。

因此本文的设计目标是：

1. 让调用方能够基于 `data_link_id` 发起稳定查询；
2. 让 `query-engine` 能通过 `datalink-engine` 获取链路元数据与存储映射；
3. 让 `query-engine` 能对接实际存储后端并执行查询；
4. 让 `query-engine` 能以统一结构返回 heartbeat 场景下的查询结果；
5. 让上层服务不再直接耦合实际存储查询细节。

## 与其他组件的边界

### 与 datalink-engine 的边界

`datalink-engine` 负责：

- `data_link_id` 真相；
- `result_table` 元数据；
- 存储映射元数据；
- 数据链路生命周期管理。

`query-engine` 只负责：

- 根据 `data_link_id` 查询这些元数据；
- 消费元数据并据此执行实际查询。

也就是说：

- `query-engine` 不创建链路；
- 不修改链路；
- 不拥有 `result_table` 语义定义。

### 与实际存储后端的边界

第一阶段的 `query-engine` 需要对接实际存储后端，例如 VictoriaMetrics。

它负责：

- 建立查询请求；
- 执行查询；
- 处理后端返回；
- 将结果转换成统一查询响应模型。

它不负责：

- 存储后端的运维管理；
- 指标写入；
- 数据采集代理管理。

### 与 rsagent 的边界

`rsagent` 负责：

- 节点侧上报 heartbeat 和其他运行事实；
- 把事实写入数据链路对应的存储通道。

`query-engine` 只负责读取这些事实数据。

`rsagent` 不需要知道：

- `query-engine` 如何组织查询；
- 上层服务如何解释查询结果。

### 与 nodemanage 的边界

`nodemanage` 负责：

- 节点主体建模；
- 节点纳管流程编排；
- 节点管理态真相；
- 对前端提供节点管理语义。

`query-engine` 不拥有节点管理态，也不负责输出基于 heartbeat 的在线/离线业务结论。

第一阶段里更合理的分工是：

- `query-engine` 提供 heartbeat 查询结果；
- `nodemanage` 周期性消费这些结果，并按 `node_id` 维度和自己的业务规则计算节点状态，再与管理态拼装展示。

### 与 job-manage 的边界

`job-manage` 负责：

- 创建作业；
- 选择节点；
- 维护任务调度状态；
- 决定某个节点是否进入执行流程。

`query-engine` 提供 heartbeat 查询能力，例如：

- 最近 heartbeat 记录；
- 某批节点的 heartbeat 查询结果；
- 某时间窗口内的 heartbeat 数据。

`query-engine` 不负责直接给出：

- `executable`
- `skipped_offline`
- `skipped_unhealthy`
- `skipped_missing_agent`

这些属于上层业务判断结果，而不是 `query-engine` 的职责。第一阶段里，`job-manage` 更合理的做法是消费 `nodemanage` 已计算好的节点状态，而不是自己直接解释 heartbeat 数据。

## 第一阶段职责

第一阶段建议只收敛到以下最小职责：

1. 根据 `data_link_id` 读取链路元数据；
2. 解析 heartbeat 场景对应的查询目标；
3. 对接实际存储后端执行查询；
4. 返回统一结构的 heartbeat 查询结果；
5. 支持单节点和批量节点查询；
6. 为 `nodemanage` 的周期性状态计算提供稳定查询输入。

第一阶段建议**不**把以下职责放进 `query-engine`：

- 在线/离线判断；
- 健康摘要解释；
- 作业执行前检查结论；
- 节点管理态聚合；
- 多业务域规则计算。

## 第一阶段范围

### 1. heartbeat 查询优先

第一阶段建议先固定 `query-engine` 的主战场是 heartbeat 场景。

这意味着：

- 对外先稳定 heartbeat 查询接口；
- 先围绕节点 heartbeat 数据组织查询模型；
- CPU、内存、磁盘、任务执行结果等其他查询能力统一留待后续扩展。

### 2. 查询优先，不做业务解释

第一阶段建议把能力收口成：

- 能查到；
- 能统一返回；
- 不负责业务解释。

这样做的价值是：

- 守住边界；
- 避免 `query-engine` 过早演化成通用业务规则中心；
- 让不同上层服务按各自语义消费相同查询结果。

## 核心领域对象

## 1. QueryTarget

`QueryTarget` 表示一次查询面向的目标对象。

第一阶段在 heartbeat 场景里，建议至少包含：

- `data_link_id`
- `node_ids`
- `time_range`
- `limit`

这里建议：

- `node_ids` 用于明确要查哪些节点；
- `time_range` 用于限定查询窗口；
- 更复杂的过滤表达式后续再扩。

## 2. ResolvedLinkMetadata

`ResolvedLinkMetadata` 是 `query-engine` 内部读取模型，用于承接从 `datalink-engine` 读取到的链路元数据。

建议至少包含：

- `data_link_id`
- `result_table_name`
- `storage_backend_type`
- `storage_locator`
- `query_hints`

这里强调：

- 这是 `query-engine` 的内部读模型；
- 不意味着 `query-engine` 拥有这些元数据的真相。

## 3. StorageQueryRequest

`StorageQueryRequest` 表示一次实际落到存储后端的查询请求。

建议至少包含：

- `backend_type`
- `resolved_table`
- `filters`
- `time_range`
- `limit`

这个对象的价值是把：

- 上层查询语义；
- 链路元数据；
- 存储查询参数

收敛成一份真正可执行的后端查询请求。

## 4. HeartbeatRecord

`HeartbeatRecord` 表示 heartbeat 场景下的一条查询结果记录。

建议至少包含：

- `node_id`
- `timestamp`
- `labels`
- `raw_value`

这里要注意：

- `HeartbeatRecord` 是查询结果记录；
- 它不是“节点在线状态对象”。

## 5. BatchHeartbeatQueryResult

`BatchHeartbeatQueryResult` 表示一次批量 heartbeat 查询的统一返回。

建议至少包含：

- `data_link_id`
- `records`
- `queried_at`
- `partial_failure`
- `warnings`

它解决的是：

- 上层服务如何消费一批节点的 heartbeat 查询结果；
- 当部分节点无结果或部分分片查询失败时，如何保留诊断信息。

## 主链路设计

第一阶段建议把主查询链路收敛成：

1. 调用方提交 `QueryTarget`；
2. `query-engine` 通过 `data_link_id` 向 `datalink-engine` 读取链路元数据；
3. `query-engine` 生成 `ResolvedLinkMetadata`；
4. `query-engine` 把业务查询目标转换为 `StorageQueryRequest`；
5. `query-engine` 调用对应的存储后端适配器执行查询；
6. `query-engine` 把返回结果转换为统一查询结果对象；
7. `nodemanage` 基于这些结果按 `node_id` 维度组织业务规则并产出节点状态。

这条链路里，`query-engine` 最重要的职责是：

- 元数据解析；
- 后端适配；
- 结果标准化。

最不应该承担的职责是：

- 业务规则解释；
- 控制面状态裁决。

## 内部模块建议

为了让后续实现和边界保持清晰，建议 `query-engine` 内部至少拆成以下模块。

### 1. MetadataResolver

负责：

- 根据 `data_link_id` 读取链路元数据；
- 把 `datalink-engine` 的响应转换为 `query-engine` 内部读模型。

### 2. QueryPlanner

负责：

- 把 `QueryTarget` 与 `ResolvedLinkMetadata` 合成为 `StorageQueryRequest`；
- 选择查询窗口、过滤条件和限制参数；
- 把 `query_template` 中的约定变量替换成实际查询参数。

### 3. StorageQueryAdapter

负责：

- 对接具体存储后端；
- 执行查询；
- 处理后端响应与错误。

第一阶段可先围绕 VictoriaMetrics 组织，但接口层面应保留未来扩展其他后端的可能性。

### 4. ResultNormalizer

负责：

- 把后端返回转换成统一查询结果结构；
- 保持字段风格一致；
- 附带必要的 warnings / partial failure 信息。

## API 设计建议

第一阶段建议对外暴露“**查询语义接口**”，而不是“业务判断接口”。

## `query_template` 约定

### 1. 模板定位

第一阶段建议把 `result_table.query_template` 明确收敛为：

- **面向实际存储后端的原生查询模板**；
- 由 `datalink-engine` 作为链路元数据的一部分保存；
- 由 `query-engine` 在运行时读取并完成变量替换。

这意味着第一阶段不建议再引入一套平台级中间查询 DSL。

### 2. 占位变量约定

heartbeat 场景下，第一阶段建议只支持以下固定占位变量：

- `$metric_name`
- `$node_id`
- `$agent_id`
- `$node_ip`
- `$start_at`
- `$end_at`

这里建议：

- 变量名集合固定；
- `query-engine` 只负责对这组已知变量做替换；
- 不支持调用方传入任意变量名。

### 3. 第一阶段允许的 heartbeat 查询类型

第一阶段建议 `query_template` 只服务于以下三类查询：

1. 单节点最近 heartbeat 查询；
2. 批量节点最近 heartbeat 查询；
3. 时间范围 heartbeat 查询。

这意味着模板需要能表达：

- metric 名；
- 节点维度过滤；
- 时间窗口过滤。

但不建议第一阶段在模板里承载：

- 通用聚合 DSL；
- 自由多字段排序；
- 报表级统计；
- 任意 label 自由查询。

### 4. 维度过滤约束

heartbeat 场景下，第一阶段建议只开放以下维度过滤：

- `node_id`
- `agent_id`
- `node_ip`

其中：

- `node_id` 是主查询维度；
- `agent_id` / `node_ip` 是补充维度；
- 更宽的 labels 自由过滤，留待后续扩展。

### 5. `query-engine` 的职责

在这套约定下，`query-engine` 第一阶段对 `query_template` 的职责应明确为：

1. 读取 `result_table.query_template`；
2. 校验所需变量是否齐备；
3. 按固定变量集合替换占位符；
4. 把模板转换成后端可执行查询；
5. 执行查询并返回统一结果。

调用方不应直接操作底层模板字符串，也不应直接拼接后端原生查询。

### 6. 示例性约定

在设计文档层，heartbeat 模板可以先理解成如下语义：

```text
query $metric_name
where node_id = $node_id
  and agent_id = $agent_id
  and node_ip = $node_ip
  and timestamp between $start_at and $end_at
```

如果后端是 VictoriaMetrics，最终落地可以是对应的原生表达；第一阶段文档更重要的是冻结变量、维度和时间窗口约定，而不是冻结某一条具体查询语句。

### 7. VictoriaMetrics 风格示例

如果 heartbeat 第一阶段实际落在 VictoriaMetrics，文档里建议把模板理解成接近如下风格：

#### 最近 heartbeat 查询

```text
max_over_time($metric_name{node_id="$node_id",agent_id="$agent_id",node_ip="$node_ip"}[$window])
```

这个表达的含义是：

- 围绕固定 `metric_name` 查询；
- 按节点维度过滤；
- 在一个窗口里获取最近有效数据。

#### 时间范围 heartbeat 查询

```text
$metric_name{node_id="$node_id",agent_id="$agent_id",node_ip="$node_ip"}
```

再由 `query-engine` 在执行层补充：

- `start_at`
- `end_at`
- step / range query 参数

### 8. 模板执行规则

为了避免模板与执行层职责混淆，第一阶段建议明确：

1. `query_template` 负责描述基础查询表达；
2. `query-engine` 负责注入 `$metric_name`、维度变量和时间窗口参数；
3. `query-engine` 负责根据调用类型决定用 instant query 还是 range query；
4. `node_id` 是主过滤维度；
5. `agent_id` / `node_ip` 缺失时可以省略对应过滤条件；
6. 第一阶段不要求模板显式覆盖所有查询类型的所有细节。

这样可以保证：

- datalink 层保存的是“怎么查这类数据”的入口模板；
- query-engine 负责把这个入口模板变成一次具体可执行查询；
- 上层服务仍然只拿查询结果，不直接接触后端原生查询。

## 1. 单节点最近 heartbeat 查询

### 目标

让上层服务按 `data_link_id + node_id` 查询最近 heartbeat 记录。

### 输入建议

- `data_link_id`
- `node_id`

### 输出建议

- `data_link_id`
- `node_id`
- `record`
- `queried_at`
- `warnings`

## 2. 批量最近 heartbeat 查询

### 目标

让上层服务一次查询多个节点的最近 heartbeat 记录。

### 输入建议

- `data_link_id`
- `node_ids`

### 输出建议

- `data_link_id`
- `records`
- `queried_at`
- `partial_failure`
- `warnings`

## 3. heartbeat 时间范围查询

### 目标

让上层服务按时间窗口读取 heartbeat 历史数据。

### 输入建议

- `data_link_id`
- `node_ids`
- `start_at`
- `end_at`
- `limit`

### 输出建议

- `records`
- `queried_at`
- `next_cursor` 或分页占位字段

### 设计约束

- 第一阶段可以先只做最基础时间范围查询；
- 更复杂的聚合、统计和多维分析留待后续。

## 错误语义建议

第一阶段建议把错误收敛成三大类：

1. **链路元数据错误**
   - `data_link_id` 不存在；
   - `result_table` 不可解析；
   - 存储映射缺失。

2. **存储查询错误**
   - 后端不可达；
   - 查询超时；
   - 后端响应格式异常。

3. **请求参数错误**
   - `node_ids` 为空；
   - 时间窗口非法；
   - 参数格式错误。

需要特别说明的是：

- `query-engine` 负责返回 heartbeat 查询结果；
- 查询最近 5 分钟时序数据后，是否视为在线，属于上层组件自己的业务规则；
- 更复杂的业务结论同样应由上层服务决定。

## 第一阶段明确不展开的内容

以下内容建议不纳入第一阶段冻结范围：

1. 在线/离线业务判断；
2. 健康摘要计算；
3. 作业执行前检查结论生成；
4. CPU / 内存 / 磁盘 / 网络等多指标解释；
5. 通用 query builder；
6. 报表、BI、统计分析；
7. 跨业务域统一规则中心。

## Assumptions / TBD 汇总

以下内容当前仍保留为后续待定：

1. heartbeat 查询时用于定位记录的主键组合，是否只用 `node_id`；
2. `ResolvedLinkMetadata.query_hints` 的最终字段结构；
3. VictoriaMetrics 之外的其他后端是否在第二阶段引入；
4. 时间范围查询的分页模型；
5. 批量查询的上限、超时和缓存策略；
6. 查询结果里的 labels / raw_value 最终标准化程度。

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/query-engine概述.md`
- `doc/nodemanage详细设计.md`
- `doc/job-manage详细设计.md`
- `doc/rsagent详细设计.md`
- `datalink-engine/README.md`

后续如果 `datalink-engine` 的链路元数据模型或实际存储后端接入方式继续演进，本文中的 `query-engine` 设计也需要同步更新，避免查询层与元数据层之间的契约发生漂移。
