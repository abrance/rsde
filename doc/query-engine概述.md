# query-engine 概述

## query-engine 是什么

`query-engine` 是节点管理体系中的查询服务组件，负责基于 datalink-engine 提供的数据链路元数据和 `result_table` 映射，对接 VictoriaMetrics 等存储后端，并向 NodeManage、前端页面或其他内部服务提供稳定的节点状态查询入口。

它不直接管理节点，也不负责安装 agent。它的核心职责是把底层指标数据转化为产品可理解的节点状态，例如在线、离线、最近心跳时间、健康状态和基础资源指标。

一句话定位：

> `query-engine` 是 NodeManage 体系的指标查询与状态解释服务，是 datalink-engine 管理的数据链路和业务产品之间的查询抽象层。

## query-engine 解决什么问题

### 1. NodeManage 不应该直接理解 VictoriaMetrics 查询细节

节点心跳和状态指标通过 datalink-engine 纳管的数据链路写入 VictoriaMetrics，但 NodeManage 的职责是节点管理和流程编排，不应该直接耦合 VM 查询语句、指标命名、时间窗口、`result_table` 解析和状态判断规则。

`query-engine` 将这些查询细节封装起来。它先通过 datalink-engine 找到对应数据链路和 `result_table`，再访问 VictoriaMetrics，让 NodeManage 只需要调用明确的业务接口，例如“查询节点是否在线”“查询最近心跳时间”“查询节点基础指标”。

### 2. 节点状态需要统一解释规则

节点是否在线不是单纯的数据库字段，而是根据 heartbeat 指标和时间窗口动态计算出来的状态。例如最近 30 秒内有心跳可以认为在线，超过阈值则认为离线或未知。

`query-engine` 负责统一这些规则，避免 NodeManage、前端、JobManage 等多个组件各自实现一套判断逻辑，导致状态不一致。

### 3. 指标系统需要对业务隐藏复杂性

VictoriaMetrics 适合存储和查询时间序列数据，但它的查询结果通常不是直接面向产品语义的。业务侧需要的是“节点状态”“最近心跳”“节点健康摘要”，而不是原始指标点。

`query-engine` 将原始指标转换成稳定的业务响应模型，降低上层服务使用指标系统的成本。

### 4. 后续多类指标查询需要统一入口

第一阶段 query-engine 主要查询 heartbeat 指标。后续可以扩展到 CPU、内存、磁盘、网络、rsagent 运行状态、任务执行状态等指标。

如果没有 query-engine，这些能力会分散在不同服务里；有了 query-engine，所有指标查询都可以通过统一服务演进。

## 与其他组件的关系

- `rsagent`：负责在节点侧上报 heartbeat 和基础指标。
- VictoriaMetrics：负责存储时间序列指标。
- datalink-engine：负责管理 heartbeat 等数据链路、唯一链路 ID、`result_table` 和存储映射。
- `query-engine`：负责查询 VictoriaMetrics，并将指标解释为业务状态。
- NodeManage：负责节点管理，通过 query-engine 获取节点在线状态和健康信息。
- JobManage：执行任务前可以通过 query-engine 判断目标节点是否在线、是否适合执行任务。

## 第一阶段目标

query-engine 第一阶段优先解决节点在线状态查询：

1. 通过 `data_link_id` 向 datalink-engine 获取 heartbeat 的 `result_table` 和 VictoriaMetrics 存储映射。
2. 支持按节点唯一标识查询最近心跳时间。
3. 支持批量查询节点在线状态。
4. 对外提供稳定 API，供 NodeManage 查询节点状态。
5. 将 datalink-engine 元数据解析和 VictoriaMetrics 查询细节封装在 query-engine 内部。

第一阶段完成后，NodeManage 不再直接依赖自身数据库判断节点是否在线，而是通过 query-engine 获取基于真实 heartbeat 指标计算出的节点状态。
