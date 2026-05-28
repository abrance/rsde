# query-engine 概述

## query-engine 是什么

`query-engine` 是节点管理体系中的查询服务组件，负责基于 datalink-engine 提供的数据链路元数据和 `result_table` 映射，对接 VictoriaMetrics 等存储后端，并向 NodeManage、前端页面或其他内部服务提供稳定的 heartbeat 数据查询入口。

它不直接管理节点，也不负责安装 agent。它的核心职责是对接实际存储后端执行 heartbeat 查询，并把 heartbeat 链路上的时序查询结果以统一结构返回给上层服务。

一句话定位：

> `query-engine` 是 NodeManage 体系的 heartbeat 数据查询服务，是 datalink-engine 管理的数据链路与实际存储后端之间的查询抽象层。

## query-engine 解决什么问题

### 1. NodeManage 不应该直接理解 VictoriaMetrics 查询细节

节点心跳和状态指标通过 datalink-engine 纳管的数据链路写入 VictoriaMetrics，但 NodeManage 的职责是节点管理和流程编排，不应该直接耦合 VM 查询语句、指标命名、时间窗口和 `result_table` 解析规则。

`query-engine` 将这些查询细节封装起来。它先通过 datalink-engine 找到对应数据链路和 `result_table`，再访问 VictoriaMetrics，让 NodeManage 只需要调用明确的 heartbeat 查询接口，例如“查询最近 heartbeat 记录”“批量查询 heartbeat 数据”“查询 heartbeat 时间范围数据”。

### 2. 实际存储查询细节需要统一封装

不同实际存储后端的查询方式、结果格式和时间范围表达往往并不一致。如果让上层服务直接面对这些差异，调用方会同时耦合数据链路元数据和存储查询细节。

`query-engine` 的价值是把这些查询差异统一封装起来，让上层服务只关心“我要查什么数据”，而不直接关心“底层怎么查”。

### 3. 查询结果需要统一返回结构

VictoriaMetrics 适合存储和查询时间序列数据，但它的查询结果通常不适合直接暴露给多个上层服务。不同调用方如果都自己适配返回结构，会造成重复逻辑和接口风格漂移。

`query-engine` 负责把查询结果收敛成统一结构，降低上层服务直接对接底层存储的成本。

### 4. 后续多类查询能力需要统一入口

第一阶段 query-engine 主要查询 heartbeat 数据。后续可以扩展到 CPU、内存、磁盘、网络、rsagent 运行状态、任务执行状态等更多查询场景。

如果没有 query-engine，这些能力会分散在不同服务里；有了 query-engine，所有查询能力都可以通过统一服务演进。

## 与其他组件的关系

- `rsagent`：负责在节点侧上报 heartbeat 和基础指标。
- VictoriaMetrics：负责存储时间序列指标。
- datalink-engine：负责管理 heartbeat 等数据链路、唯一链路 ID、`result_table` 和存储映射。
- `query-engine`：负责查询 VictoriaMetrics 等实际存储后端，并返回 heartbeat 链路查询结果。
- NodeManage：负责节点管理，周期性通过 query-engine 获取 heartbeat 数据，并按 `node_id` 维度计算节点状态。
- JobManage：消费 NodeManage 提供的节点状态，而不是自己直接解释 heartbeat 数据。

## 第一阶段目标

query-engine 第一阶段优先解决 heartbeat 数据查询：

1. 通过 `data_link_id` 向 datalink-engine 获取 heartbeat 的 `result_table` 和 VictoriaMetrics 存储映射。
2. 支持按节点唯一标识查询最近 heartbeat 记录。
3. 支持批量查询节点 heartbeat 数据。
4. 对外提供稳定 API，供 NodeManage 周期性查询 heartbeat 数据。
5. 将 datalink-engine 元数据解析和 VictoriaMetrics 查询细节封装在 query-engine 内部。

第一阶段完成后，NodeManage 不再直接依赖自身实现去对接底层存储，而是通过 query-engine 获取 heartbeat 时序数据，再按自己的规则计算节点状态。
