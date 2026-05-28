# datalink-engine 概述

## datalink-engine 是什么

`datalink-engine` 是平台中的数据链路管理服务，负责统一管理“数据从哪里来、写到哪里去、如何被查询”的链路元数据。

在节点管理体系中，`rsagent` 上报 heartbeat 指标到 VictoriaMetrics 不是一条隐含链路，而应该是一条被 datalink-engine 创建和纳管的数据链路。链路创建后会生成唯一的数据链路 ID，并绑定对应的 `result_table`，上层服务通过 `result_table` 找到实际的数据存储位置和查询方式。

一句话定位：

> `datalink-engine` 是平台的数据链路登记、元数据管理和 result_table 映射服务。

## 核心概念

### 1. 数据链路

数据链路描述一类数据从生产端到存储端的完整路径，例如：

- `rsagent` 上报节点 heartbeat 指标到 VictoriaMetrics。
- `rsagent` 上报节点基础资源指标到 VictoriaMetrics。
- job-manage 写入任务执行结果到某个存储系统。

每条数据链路都需要有唯一 ID，并包含 `datasource`、`etl_pipeline` 和 `result_table` 三类核心元数据：

- `datasource` 描述数据如何被采集。
- `etl_pipeline` 描述数据在入库前如何透传、清洗、转换和标准化；它是必选对象。
- `result_table` 描述数据最终如何被查询。

### 2. datasource

`datasource` 是数据链路的数据源定义，用于记录这条链路的采集方式元信息。它回答的是“数据从哪里来、用什么方式采、采集端是谁、采集周期是什么、采集格式是什么”。

`datasource` 需要至少包含两个标准字段：

```text
data_type      = metric | log | trace | event | profile
collect_method = agent | push | pull | ebpf | file | http | kafka
```

例如节点 heartbeat 链路的 `datasource` 可以记录：

- 数据生产端：`rsagent`。
- 数据类型：`data_type = metric`。
- 采集方式：`collect_method = agent`，表示由 agent 主动采集或上报。
- 上报协议：HTTP remote write、Prometheus exposition、或者平台自定义协议。
- 采集周期：例如每 15 秒上报一次 heartbeat。
- 关键维度：节点唯一标识、agent ID、集群/环境信息。
- 鉴权方式：token、证书或其他认证信息引用。

`datasource` 不负责保存数据本身，而是保存采集链路所需的元信息，让平台知道这条链路的数据如何产生。

### 3. 数据类型

数据类型用于描述这条数据链路采集和管理的数据形态。第一阶段建议先将数据类型建模为：

- `metric`：指标数据，例如节点 heartbeat、CPU、内存、磁盘、网络指标。
- `log`：日志数据，例如 rsagent 运行日志、任务执行日志、系统日志。
- `trace`：调用链路数据，用于描述一次请求或任务在多个组件之间的流转。
- `event`：事件数据，例如节点上线、节点离线、任务开始、任务失败、配置变更。
- `profile`：性能剖析数据，例如 CPU profile、memory profile、goroutine/thread profile 等。

`ebpf` 不建议直接和 `metric`、`log`、`trace`、`event`、`profile` 完全等价建模。更准确地说，eBPF 是一种采集技术或采集来源，它可以产出 metric、event、trace、profile 等多种数据类型。

因此推荐模型是：

- `data_type` 表示数据形态：metric、log、trace、event、profile。
- `collect_method` 表示采集方式：agent、push、pull、ebpf、file、http、kafka。

如果产品上希望突出 eBPF 能力，可以在展示层把 eBPF 作为链路类型入口，但底层元数据仍建议拆成 `data_type + collect_method`，避免后续无法表达“通过 eBPF 采集 metric”或“通过 eBPF 采集 event”这类场景。

### 4. result_table

`result_table` 是数据链路对外暴露的数据结果标识。它不一定等同于传统数据库表，而是一个逻辑结果表，用来描述数据的查询入口和存储位置。

通过 `result_table`，上层服务可以解决“这类数据存在哪里、应该怎么查”的问题。例如节点 heartbeat 链路的 `result_table` 可以映射到 VictoriaMetrics 中某组指标名称和标签约束。

### 5. 数据链路 ID

数据链路 ID 是每条链路的唯一标识。它用于在平台内引用一条链路，例如：

- rsagent 上报 heartbeat 时关联链路 ID，并遵循该链路 `datasource` 中定义的采集方式。
- query-engine 查询节点状态时，通过链路 ID 找到对应的 `result_table`。
- NodeManage 展示节点状态时，可以追溯状态来自哪条数据链路。

### 6. etl_pipeline

`etl_pipeline` 描述数据链路中的入库前处理过程，用于定义数据是直接透传入库，还是经过清洗、转换、标准化后再入库。

典型能力包括：

- 数据清洗：去重、补默认值、字段修正。
- 数据转换：字段重命名、类型转换、单位归一化。
- 数据标准化：标签规范化、时间戳对齐、业务域映射。
- 数据过滤：无效数据丢弃、异常值过滤、采样策略。
- 数据增强：补充维度字段（例如 cluster/env/domain）。

`etl_pipeline` 是数据链路的必选组成部分。即使不清洗、直接入库，也必须显式声明 ETL 管道，并通过 `mode = passthrough` 表达“直接透传入库”。第一阶段建议先支持最小模型：

- `pipeline_id`
- `mode`（`passthrough` / `vector`）
- `config`（`map<string,string>`）
- `version`

在 heartbeat 场景中，如果不需要清洗，则使用 `passthrough`；如果需要通过 Vector 做清洗、标准化、重打标签或路由，则使用 `vector`，并把完整处理配置放入 `config`。

## 逻辑概念字段

## V1 最终字段总表

为了便于后续直接落数据库表结构、Rust struct 和 API DTO，V1 推荐以以下字段集作为最终收敛版本。

### 1. data_link

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `data_link_id` | string | 是 | 数据链路唯一 ID。 |
| `name` | string | 是 | 数据链路名称。 |
| `description` | string | 否 | 数据链路说明。 |
| `domain` | string | 是 | 所属业务域。 |
| `owner_service` | string | 是 | 创建或拥有该链路的服务。 |
| `data_type` | enum | 是 | 数据形态：`metric`、`log`、`trace`、`event`、`profile`。 |
| `datasource_id` | string | 是 | 关联的 datasource ID。 |
| `etl_pipeline_id` | string | 是 | 关联的 ETL 管道 ID。 |
| `result_table_id` | string | 是 | 关联的 result_table ID。 |
| `result_table_name` | string | 是 | 逻辑结果表名，全局唯一。 |
| `status` | enum | 是 | `draft`、`active`、`disabled`、`deleted`。 |
| `status_message` | string | 否 | 对非 `active` 状态的说明文字。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 2. datasource

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `datasource_id` | string | 是 | datasource 唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `producer` | string | 是 | 数据生产端，例如 `rsagent`。 |
| `data_type` | enum | 是 | `metric`、`log`、`trace`、`event`、`profile`。 |
| `collect_method` | enum | 是 | `agent`、`push`、`pull`、`ebpf`、`file`、`http`、`kafka`。 |
| `protocol` | string | 否 | 上报或采集协议，例如 `remote_write`。 |
| `interval_seconds` | u64 | 否 | 周期性采集间隔。 |
| `labels` | map | 否 | 固定标签。 |
| `dimension_keys` | list<string> | 是 | 维度字段。 |
| `auth_ref` | string | 否 | 鉴权信息引用。 |
| `config` | map | 否 | 采集方式扩展配置。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 3. etl_pipeline

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `etl_pipeline_id` | string | 是 | ETL 管道唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `pipeline_name` | string | 是 | ETL 管道名称。 |
| `mode` | enum | 是 | `passthrough` 或 `vector`。 |
| `config` | map<string,string> | 否 | ETL 完整配置；`vector` 模式下承载全部清洗/转换/路由内容。 |
| `version` | string | 是 | 管道版本。 |
| `enabled` | bool | 是 | 是否启用。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 4. result_table

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `result_table_id` | string | 是 | result_table 唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `result_table_name` | string | 是 | 逻辑结果表名，全局唯一。 |
| `storage_type` | enum | 是 | `victoriametrics`、`mysql`。 |
| `storage_cluster` | string | 否 | 存储集群或实例标识。 |
| `database` | string | 否 | 数据库或命名空间。 |
| `table_name` | string | 否 | 物理表名。 |
| `metric_name` | string | 否 | 指标名。 |
| `query_template` | string | 否 | 查询模板。 |
| `schema` | map | 否 | 字段或标签定义。 |
| `retention_days` | u32 | 否 | 数据保留时间。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 1. data_link

`data_link` 是数据链路的主对象，用来描述一条链路的基本身份、业务归属和生命周期。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `data_link_id` | string | 是 | 数据链路唯一 ID，全局唯一。 |
| `name` | string | 是 | 数据链路名称，例如 `node_heartbeat`。 |
| `description` | string | 否 | 数据链路说明。 |
| `domain` | string | 是 | 所属业务域，例如 `nodemanage`、`job-manage`。 |
| `owner_service` | string | 是 | 创建或拥有该链路的服务，例如 `nodemanage`。 |
| `data_type` | enum | 是 | 数据形态：`metric`、`log`、`trace`、`event`、`profile`。 |
| `datasource_id` | string | 是 | 关联的 datasource ID。 |
| `result_table_id` | string | 是 | 关联的 result_table ID。 |
| `result_table_name` | string | 是 | 关联的 result_table 唯一名称；可由客户端指定，未指定时系统自动生成。 |
| `etl_pipeline_id` | string | 是 | 关联的 ETL 管道 ID；无清洗时也应绑定默认 passthrough 管道。 |
| `status` | enum | 是 | 链路状态：`draft`、`active`、`disabled`、`deleted`。 |
| `status_message` | string | 否 | 对 `status` 的补充说明字段，仅用于解释链路为何当前不可用或非 `active`；不作为机器判断依据。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 2. datasource

`datasource` 描述数据采集方式和采集端元信息，回答“这条链路的数据如何产生”。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `datasource_id` | string | 是 | datasource 唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `producer` | string | 是 | 数据生产端，例如 `rsagent`、`job-manage`。 |
| `data_type` | enum | 是 | `metric`、`log`、`trace`、`event`、`profile`。 |
| `collect_method` | enum | 是 | `agent`、`push`、`pull`、`ebpf`、`file`、`http`、`kafka`。 |
| `protocol` | string | 否 | 上报或采集协议，例如 `remote_write`、`http`、`prometheus`。 |
| `interval_seconds` | u64 | 否 | 周期性采集间隔；事件类数据可为空。 |
| `labels` | map | 否 | 链路级固定标签，例如环境、集群、业务域。 |
| `dimension_keys` | list<string> | 是 | 数据维度字段，例如 `node_id`、`agent_id`。 |
| `auth_ref` | string | 否 | 鉴权信息引用，不直接保存明文 token 或密钥。 |
| `config` | map | 否 | 采集方式相关扩展配置。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 3. result_table

`result_table` 描述数据结果的逻辑查询入口和实际存储映射，回答“这条链路的数据最终在哪里查”。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `result_table_id` | string | 是 | result_table 唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `result_table_name` | string | 是 | 逻辑结果表名称，全局唯一，例如 `rt_node_heartbeat`；客户端可指定，未指定时系统自动生成。 |
| `storage_type` | enum | 是 | 存储类型，例如 `victoriametrics`、`mysql`、`object_storage`、`kafka`。 |
| `storage_cluster` | string | 否 | 存储集群或实例标识。 |
| `database` | string | 否 | 数据库或命名空间；对 VM 可表示 tenant/account。 |
| `table_name` | string | 否 | 物理表名；对指标系统可为空。 |
| `metric_name` | string | 否 | 指标名，例如 `rsagent_node_heartbeat`。 |
| `query_template` | string | 否 | 查询模板，例如 PromQL/SQL 模板。 |
| `schema` | map | 否 | 字段定义或指标标签定义。 |
| `retention_days` | u32 | 否 | 数据保留时间。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 4. etl_pipeline

`etl_pipeline` 描述该数据链路在采集后、入库前的数据清洗与转换流程。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `etl_pipeline_id` | string | 是 | ETL 管道唯一 ID。 |
| `data_link_id` | string | 是 | 所属数据链路 ID。 |
| `pipeline_name` | string | 是 | ETL 管道名称，例如 `pl_node_heartbeat_std`。 |
| `mode` | enum | 是 | `passthrough` 或 `vector`。 |
| `version` | string | 是 | 管道版本，例如 `v1`。 |
| `config` | map<string,string> | 否 | 管道配置；`passthrough` 模式通常为空，`vector` 模式下包含完整清洗/路由配置。 |
| `enabled` | bool | 是 | 是否启用。 |
| `created_at` | datetime | 是 | 创建时间。 |
| `updated_at` | datetime | 是 | 更新时间。 |

### 5. 节点 heartbeat 链路示例

```yaml
data_link:
  data_link_id: dl_node_heartbeat
  name: node_heartbeat
  domain: nodemanage
  owner_service: nodemanage
  data_type: metric
  datasource_id: ds_node_heartbeat
  etl_pipeline_id: pl_node_heartbeat_std
  result_table_id: rt_node_heartbeat
  result_table_name: rt_node_heartbeat
  status: active

datasource:
  datasource_id: ds_node_heartbeat
  data_link_id: dl_node_heartbeat
  producer: rsagent
  data_type: metric
  collect_method: agent
  protocol: remote_write
  interval_seconds: 15
  dimension_keys:
    - node_id
    - agent_id
  labels:
    domain: nodemanage

etl_pipeline:
  etl_pipeline_id: pl_node_heartbeat_std
  data_link_id: dl_node_heartbeat
  pipeline_name: pl_node_heartbeat_std
  mode: vector
  version: v1
  enabled: true
  config:
    inputs: heartbeat_source
    transforms: normalize_ts,add_domain_label
    sink: vm_remote_write

result_table:
  result_table_id: rt_node_heartbeat
  data_link_id: dl_node_heartbeat
  result_table_name: rt_node_heartbeat
  storage_type: victoriametrics
  storage_cluster: vm-default
  metric_name: rsagent_node_heartbeat
  query_template: 'max_over_time(rsagent_node_heartbeat{node_id="$node_id"}[5m])'
  schema:
    value: heartbeat timestamp or 1
    labels:
      - node_id
      - agent_id
      - domain
```

## datalink-engine 解决什么问题

### 1. 避免数据写入链路散落在各组件中

如果每个组件都自行决定数据写到哪里、指标叫什么、查询时怎么拼条件，系统会很快变得不可维护。

`datalink-engine` 将数据链路集中登记和管理，让数据生产、采集方式、存储位置和查询入口都有明确的元数据入口。

### 2. 让上层服务通过 result_table 找到数据位置

NodeManage、query-engine、job-manage 不应该硬编码每类数据的采集方式和存储位置。它们应该通过数据链路获取 `datasource`，通过 `result_table` 获取数据位置和查询方式。

这样当底层存储从 VictoriaMetrics 扩展到 MySQL、对象存储、日志系统或其他存储时，上层服务不需要大规模改造。

### 3. 让节点 heartbeat 链路可追踪、可治理

节点 heartbeat 是判断节点在线状态的关键数据，不能只是“rsagent 往某个 VM 地址写指标”。

它应该作为一条正式数据链路被创建、命名、授权、记录和查询。这样才能支撑后续的问题排查、链路迁移、指标治理和权限控制。

### 4. 支撑未来更多数据链路扩展

第一阶段只有节点 heartbeat 链路，后续还会有节点资源指标、任务执行结果、文件传输结果、日志采集结果等链路。

有了 datalink-engine，这些链路可以用统一模型管理，而不是每个业务组件重复造一套链路元数据。

## 与其他组件的关系

- `rsagent`：数据生产端，负责按照数据链路定义上报 heartbeat、资源指标或任务结果。
- VictoriaMetrics：一种具体的数据存储后端，保存 heartbeat 等时间序列指标。
- datalink-engine：负责创建和管理数据链路、链路 ID、`datasource`、`result_table` 和存储映射。
- query-engine：查询前通过 `data_link_id` 向 datalink-engine 获取链路元数据和 `result_table`，再访问 VictoriaMetrics 等后端。
- NodeManage：创建节点相关链路时请求 datalink-engine，并通过 query-engine 获取 heartbeat 数据后自行计算节点状态。
- job-manage：创建任务结果链路或查询任务结果时，可以复用 datalink-engine 的链路管理能力。

## 第一阶段目标

datalink-engine 第一阶段优先服务节点 heartbeat 链路：

1. 支持创建数据链路，生成唯一数据链路 ID。
2. 支持为数据链路绑定 `datasource`，记录 heartbeat 的采集方式元信息。
3. 支持为数据链路绑定 `result_table`。
4. 支持登记 heartbeat 链路对应的 VictoriaMetrics 存储位置、指标名称和标签维度。
5. 支持 query-engine 根据 `data_link_id` 获取查询所需元数据；`result_table_name` 仅作为管理检索入口。
6. 支持 NodeManage 在创建节点 heartbeat 链路时请求 datalink-engine，而不是由 NodeManage 或 rsagent 隐式决定链路。

第一阶段完成后，节点 heartbeat 不再是散落在 rsagent 和 VictoriaMetrics 之间的隐式指标写入，而是一条可被平台识别、查询和治理的数据链路。

## 已明确设计决策

当前文档已经定义了 datalink-engine 的核心定位、`data_link`、`datasource`、`etl_pipeline`、`result_table` 以及 heartbeat 链路示例。以下内容作为当前阶段的明确设计决策。

### 1. 字段约束和命名规范

字段约束如下：

- `data_link_id`、`datasource_id`、`result_table_id` 默认由系统生成；客户端不需要关心生成规则。
- `result_table_name` 是 result_table 表中的唯一键，全局唯一。
- `result_table_name` 可以由客户端创建链路时指定；如果客户端未指定，则由 datalink-engine 自动生成。
- `domain`、`owner_service`、`storage_type` 统一使用小写 kebab-case 或 snake_case；同一服务内保持一种风格，当前文档示例使用 snake_case。
- `data_type` 暂定为 `metric`、`log`、`trace`、`event`、`profile`。
- `collect_method` 暂定为 `agent`、`push`、`pull`、`ebpf`、`file`、`http`、`kafka`。
- `data_type` 和 `collect_method` 当前阶段暂不设计动态扩展流程，后续按产品需要再补充。

### 2. 生命周期状态流转

生命周期状态流转定义如下：

- `draft`：链路已创建，但 datasource 或 result_table 还未完全就绪；此时不应被 rsagent 用于正式上报。
- `active`：链路已生效，rsagent 可以按 datasource 配置上报数据，query-engine 可以基于 result_table 查询数据。
- `disabled`：链路被禁用。rsagent 应停止该链路的新数据上报；query-engine 仍允许查询历史数据。
- `deleted`：链路被删除，采用软删除语义，保留审计和历史引用，不再允许新增写入。
- `status_message`：用于补充说明链路为何不可用或为何不处于 `active`，例如“waiting for Vector pipeline deployment”或“disabled by operator during maintenance”；该字段是人类可读说明，不承担状态判定语义。
- 允许流转：`draft -> active`、`active -> disabled`、`disabled -> active`、`draft/active/disabled -> deleted`。
- 不允许流转：`deleted -> active`。

建议补充约束：

- `draft`：`status_message` 可选，用于说明尚未就绪原因。
- `active`：`status_message` 应为空值（例如 `null`）。
- `disabled`：`status_message` 应为非空，说明禁用原因。
- `deleted`：`status_message` 可选；是否保留删除原因由实现决定。

### 3. API 能力边界

datalink-engine API 采用声明式风格。客户端提交期望状态，由 datalink-engine 负责创建、更新或对齐实际状态。

第一阶段 API 建议：

- `ApplyDataLink`：声明式创建或更新数据链路。请求体包含 data_link、datasource、etl_pipeline、result_table 的期望配置；其中 `etl_pipeline` 必填。
- `GetDataLink`：通过 `data_link_id` 查询数据链路详情。
- `GetDataLinkByResultTableName`：通过 `result_table_name` 查询数据链路详情。
- `SetDataLinkStatus`：启用、禁用或删除数据链路。
- `ListDataLinks`：按 domain、owner_service、data_type、status 等条件查询链路列表。

声明式 API 的目标是让客户端表达“我需要这样一条链路”，而不是要求客户端关心内部创建 datasource、result_table 的具体顺序。

### 4. 权限和租户模型

权限和租户模型暂作为 TODO 项。

功能实现后，将由统一用户鉴权组件和 API 网关组件管理 API 访问权限。datalink-engine 当前阶段只保留权限接入点，不在本组件内展开完整鉴权体系。

### 5. 与 query-engine 的接口契约

接口契约如下：

- datalink-engine 以外的组件都通过 `data_link_id` 查询链路信息。
- query-engine 通过 `data_link_id` 获取 datasource、etl_pipeline、result_table、storage 映射和 query_template。
- `result_table_name` 可作为管理检索入口，但组件间稳定引用优先使用 `data_link_id`。
- `query_template` 中变量采用 `$variable` 形式，例如 `$node_id`。
- result_table schema 需要描述指标值、标签字段、时间字段或存储后端所需的等价字段。

### 6. 与 rsagent 的链路下发方式

链路下发方式定义如下：

- NodeManage 中会有一张表维护自己依赖的数据链路元信息。
- NodeManage 第一次启动时，会向 datalink-engine 申请创建所需 datalink，例如节点 heartbeat 链路。
- 安装 rsagent 时，NodeManage 会下发配置文件，配置文件中包含 heartbeat 的 `data_link_id`。
- datasource 配置在安装 rsagent 时下发，作为安装步骤的一部分。
- rsagent 会定期通过 datalink-engine 查询 `data_link_id` 对应的链路信息，例如每 5 分钟查询一次。
- 链路配置变更后，rsagent 通过定期查询感知变化，并按新配置更新上报行为。

### 7. 存储后端抽象

当前阶段只考虑两类存储后端：VictoriaMetrics 和 MySQL。

- `victoriametrics`：用于 metric 类时间序列数据，例如 heartbeat、资源指标。
- `mysql`：用于结构化关系数据，例如链路元数据、任务结果索引、管理类记录。
- 其他存储后端如对象存储、Kafka 暂不进入第一阶段设计。
- 存储后端差异字段暂放入 result_table 的 `config` 或 `schema` 中，后续实现时再细化。

### 8. 数据治理和可观测性

各组件统一暴露标准 `/metric` API 供采集系统采集。

datalink-engine 后续需要暴露自身运行指标，例如：

- datalink 创建/更新/删除次数。
- datalink apply 成功率和失败率。
- datasource/result_table 查询次数。
- datalink-engine API 请求耗时。
- datalink 配置同步或校验失败次数。

审计记录、链路健康检查、result_table 可查询验证等能力作为后续增强项。
