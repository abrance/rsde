# datalink-engine API 契约（V1）

## 1. 范围

本契约覆盖第一阶段 datalink-engine 的核心 API：

- `ApplyDataLink`（声明式创建/更新）
- `GetDataLink`
- `GetDataLinkByResultTableName`
- `SetDataLinkStatus`
- `ListDataLinks`

目标：让 `nodemanage`、`query-engine`、`job-manage` 等组件通过 `data_link_id` 稳定查询链路元信息；`result_table_name` 作为管理检索入口。

---

## 2. 统一约定

### 2.1 API 风格

- 协议：HTTP JSON（REST 风格）
- Base Path：`/api/datalink/v1`
- 鉴权：当前阶段预留（由统一鉴权组件 + API 网关接管）

### 2.2 时间和 ID

- 时间字段使用 RFC3339 UTC 字符串
- 系统生成 ID（默认）：
  - `data_link_id`
  - `datasource_id`
  - `etl_pipeline_id`
  - `result_table_id`

### 2.3 result_table_name 规则

- `result_table_name` 全局唯一
- 客户端可指定
- 未指定时由系统自动生成

### 2.4 外部查询口径

- datalink-engine 以外组件统一通过 `data_link_id` 查询链路
- `result_table_name` 仅用于管理检索

### 2.5 枚举

```text
data_type      = metric | log | trace | event | profile
collect_method = agent | push | pull | ebpf | file | http | kafka
etl_mode       = passthrough | vector
status         = draft | active | disabled | deleted
storage_type   = victoriametrics | mysql
```

---

## 3. 数据模型（API 视角）

## 3.1 DataLinkSpec（声明式输入）

```json
{
  "name": "node_heartbeat",
  "description": "Node heartbeat metric link",
  "domain": "nodemanage",
  "owner_service": "nodemanage",
  "data_type": "metric",
  "status": "active",
  "status_message": null,
  "datasource": {
    "producer": "rsagent",
    "data_type": "metric",
    "collect_method": "agent",
    "protocol": "remote_write",
    "interval_seconds": 15,
    "labels": {"domain": "nodemanage"},
    "dimension_keys": ["node_id", "agent_id"],
    "auth_ref": "secret://rsagent/heartbeat-token",
    "config": {}
  },
  "etl_pipeline": {
    "pipeline_name": "pl_node_heartbeat_std",
    "mode": "vector",
    "version": "v1",
    "enabled": true,
    "config": {
      "inputs": "heartbeat_source",
      "transforms": "normalize_ts,add_domain_label",
      "sink": "vm_remote_write"
    }
  },
  "result_table": {
    "result_table_name": "rt_node_heartbeat",
    "storage_type": "victoriametrics",
    "storage_cluster": "vm-default",
    "metric_name": "rsagent_node_heartbeat",
    "query_template": "max_over_time(rsagent_node_heartbeat{node_id=\"$node_id\"}[5m])",
    "schema": {
      "value": "heartbeat timestamp or 1",
      "labels": ["node_id", "agent_id", "domain"]
    },
    "retention_days": 30
  }
}
```

### 3.2 DataLink（读取输出）

```json
{
  "data_link_id": "dl_node_heartbeat",
  "name": "node_heartbeat",
  "description": "Node heartbeat metric link",
  "domain": "nodemanage",
  "owner_service": "nodemanage",
  "data_type": "metric",
  "status": "active",
  "status_message": null,
  "datasource_id": "ds_node_heartbeat",
  "etl_pipeline_id": "pl_node_heartbeat_std",
  "result_table_id": "rt_id_node_heartbeat",
  "result_table_name": "rt_node_heartbeat",
  "datasource": {
    "datasource_id": "ds_node_heartbeat",
    "producer": "rsagent",
    "data_type": "metric",
    "collect_method": "agent",
    "protocol": "remote_write",
    "interval_seconds": 15,
    "labels": {"domain": "nodemanage"},
    "dimension_keys": ["node_id", "agent_id"],
    "auth_ref": "secret://rsagent/heartbeat-token",
    "config": {},
    "created_at": "2026-05-20T02:00:00Z",
    "updated_at": "2026-05-20T02:00:00Z"
  },
  "etl_pipeline": {
    "etl_pipeline_id": "pl_node_heartbeat_std",
    "pipeline_name": "pl_node_heartbeat_std",
    "mode": "vector",
    "version": "v1",
    "enabled": true,
    "config": {
      "inputs": "heartbeat_source",
      "transforms": "normalize_ts,add_domain_label",
      "sink": "vm_remote_write"
    },
    "created_at": "2026-05-20T02:00:00Z",
    "updated_at": "2026-05-20T02:00:00Z"
  },
  "result_table": {
    "result_table_id": "rt_id_node_heartbeat",
    "result_table_name": "rt_node_heartbeat",
    "storage_type": "victoriametrics",
    "storage_cluster": "vm-default",
    "metric_name": "rsagent_node_heartbeat",
    "query_template": "max_over_time(rsagent_node_heartbeat{node_id=\"$node_id\"}[5m])",
    "schema": {
      "value": "heartbeat timestamp or 1",
      "labels": ["node_id", "agent_id", "domain"]
    },
    "retention_days": 30,
    "created_at": "2026-05-20T02:00:00Z",
    "updated_at": "2026-05-20T02:00:00Z"
  },
  "created_at": "2026-05-20T02:00:00Z",
  "updated_at": "2026-05-20T02:00:00Z"
}
```

说明：

- `DataLinkSpec` 必须包含 `datasource`、`etl_pipeline`、`result_table` 三类核心声明对象。
- 即使不清洗、直接入库，`etl_pipeline` 也必须存在，并通过 `mode = passthrough` 表达透传入库。
- 当需要通过 Vector 做清洗、转换或路由时，`etl_pipeline.mode` 使用 `vector`。
- `status_message` 为可选字段，仅作为人类可读说明，不参与机器状态判断。
- 当 `status = active` 时，`status_message` 应为 `null`。
- 当 `status = draft` 时，`status_message` 可选，用于说明尚未就绪原因。
- 当 `status = disabled` 时，`status_message` 应为非空，用于说明禁用原因。
- 当 `status = deleted` 时，`status_message` 可选，是否返回删除原因由实现决定。

---

## 4. API 详细定义

## 4.1 ApplyDataLink（声明式）

### Endpoint

`PUT /api/datalink/v1/datalinks:apply`

### 语义

声明期望状态，服务端执行 create/update/reconcile：

- 如果链路不存在：创建
- 如果链路存在：按输入对齐（更新可变字段）
- 保持幂等：相同请求重复提交返回同一最终状态

### 幂等键

优先级：

1. `idempotency_key`（header：`X-Idempotency-Key`）
2. `domain + owner_service + name`
3. 若请求里有 `result_table_name`，需满足全局唯一约束

### Request

```json
{
  "spec": {
    "name": "node_heartbeat",
    "description": "Node heartbeat metric link",
    "domain": "nodemanage",
    "owner_service": "nodemanage",
    "data_type": "metric",
    "status": "active",
    "status_message": null,
    "datasource": {
      "producer": "rsagent",
      "data_type": "metric",
      "collect_method": "agent",
      "protocol": "remote_write",
      "interval_seconds": 15,
      "labels": {"domain": "nodemanage"},
      "dimension_keys": ["node_id", "agent_id"],
      "auth_ref": "secret://rsagent/heartbeat-token",
      "config": {}
    },
    "etl_pipeline": {
      "pipeline_name": "pl_node_heartbeat_std",
      "mode": "vector",
      "version": "v1",
      "enabled": true,
      "config": {
        "inputs": "heartbeat_source",
        "transforms": "normalize_ts,add_domain_label",
        "sink": "vm_remote_write"
      }
    },
    "result_table": {
      "result_table_name": "rt_node_heartbeat",
      "storage_type": "victoriametrics",
      "storage_cluster": "vm-default",
      "metric_name": "rsagent_node_heartbeat",
      "query_template": "max_over_time(rsagent_node_heartbeat{node_id=\"$node_id\"}[5m])",
      "schema": {
        "value": "heartbeat timestamp or 1",
        "labels": ["node_id", "agent_id", "domain"]
      },
      "retention_days": 30
    }
  }
}
```

### Response

```json
{
  "success": true,
  "operation": "upsert",
  "data": {
    "data_link_id": "dl_node_heartbeat",
    "etl_pipeline_id": "pl_node_heartbeat_std",
    "result_table_name": "rt_node_heartbeat",
    "status": "active"
  }
}
```

---

## 4.2 GetDataLink

### Endpoint

`GET /api/datalink/v1/datalinks/{data_link_id}`

### Response

返回完整 `DataLink` 对象。

---

## 4.3 GetDataLinkByResultTableName

### Endpoint

`GET /api/datalink/v1/datalinks/by-result-table/{result_table_name}`

### 说明

用于管理检索和排障；业务组件正常运行优先通过 `data_link_id`。

---

## 4.4 SetDataLinkStatus

### Endpoint

`PATCH /api/datalink/v1/datalinks/{data_link_id}/status`

### Request

```json
{
  "status": "disabled",
  "reason": "manual maintenance",
  "status_message": "disabled by operator during maintenance"
}
```

### 状态流转约束

- 允许：
  - `draft -> active`
  - `active -> disabled`
  - `disabled -> active`
  - `draft|active|disabled -> deleted`
- 禁止：`deleted -> active`
- 当流转到 `active` 时，服务端应清空 `status_message`。
- 当流转到 `disabled` 时，服务端应要求非空 `status_message`（或等价 `reason` 字段并映射入 `status_message`）。

---

## 4.5 ListDataLinks

### Endpoint

`GET /api/datalink/v1/datalinks?domain=nodemanage&owner_service=nodemanage&data_type=metric&status=active&page=1&page_size=20`

### 说明

支持按以下条件过滤：

- `domain`
- `owner_service`
- `data_type`
- `status`
- `storage_type`

列表项返回 `DataLink` 精简视图，至少包含：

- `data_link_id`
- `name`
- `domain`
- `owner_service`
- `data_type`
- `status`
- `datasource_id`
- `etl_pipeline_id`
- `result_table_id`
- `result_table_name`
- `updated_at`

---

## 5. 错误码

统一响应结构：

```json
{
  "success": false,
  "error": {
    "code": "DL_RESULT_TABLE_NAME_CONFLICT",
    "message": "result_table_name already exists",
    "details": {}
  }
}
```

建议错误码：

- `DL_INVALID_ARGUMENT`：参数错误
- `DL_ENUM_NOT_SUPPORTED`：枚举值不支持
- `DL_RESULT_TABLE_NAME_CONFLICT`：`result_table_name` 全局唯一冲突
- `DL_ETL_PIPELINE_INVALID`：ETL 管道定义不合法（如 `mode` 与 `config` 不匹配）
- `DL_ETL_MODE_NOT_SUPPORTED`：ETL 模式不支持
- `DL_NOT_FOUND`：链路不存在
- `DL_STATUS_TRANSITION_INVALID`：状态流转不合法
- `DL_STATUS_MESSAGE_REQUIRED`：`disabled` 状态缺少原因说明
- `DL_STATUS_MESSAGE_INVALID`：`status_message` 为空白字符串、与状态不匹配，或在 `active` 状态下错误传入非空值
- `DL_IDEMPOTENCY_CONFLICT`：幂等键冲突
- `DL_BACKEND_NOT_SUPPORTED`：存储后端不支持（第一阶段仅 VM/MySQL）
- `DL_INTERNAL_ERROR`：服务内部错误

---

## 6. 组件协作契约（第一阶段）

- NodeManage 首次启动：调用 `ApplyDataLink` 创建 heartbeat 链路，保存 `data_link_id`。
- 安装 rsagent：下发包含 `data_link_id` 的配置文件。
- rsagent 运行中：
  - 按 datasource 上报 heartbeat 到 VM
  - 每 5 分钟通过 `data_link_id` 向 datalink-engine 拉取链路配置
- query-engine：通过 `data_link_id` 获取 datasource/etl_pipeline/result_table/storage 映射并查询 heartbeat 数据。

---

## 7. 可观测性契约

所有组件统一暴露 `/metric` 接口；datalink-engine 至少暴露：

- `datalink_apply_total`
- `datalink_apply_failed_total`
- `datalink_get_total`
- `datalink_status_transition_total`
- `datalink_api_latency_ms`

并按 `domain`、`owner_service`、`status`、`storage_type` 提供必要标签。
