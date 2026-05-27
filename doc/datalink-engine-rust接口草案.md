# datalink-engine Rust 接口草案（V1）

本草案用于把当前已收敛的 V1 schema 直接映射为可实现的 Rust 代码骨架。

目标：

- 让 `datalink-engine` 可以按 `nodemanage` 风格快速落地 `models + repository + service`。
- 保证与现有文档一致：
  - `etl_pipeline` 必选
  - `mode = passthrough | vector`
  - `config = map<string,string>`
  - 无 `steps`、无 `metadata`

---

## 1. models.rs 草案

```rust
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type StringMap = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Metric,
    Log,
    Trace,
    Event,
    Profile,
}

impl DataType {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "metric" => Some(Self::Metric),
            "log" => Some(Self::Log),
            "trace" => Some(Self::Trace),
            "event" => Some(Self::Event),
            "profile" => Some(Self::Profile),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CollectMethod {
    Agent,
    Push,
    Pull,
    Ebpf,
    File,
    Http,
    Kafka,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataLinkStatus {
    Draft,
    Active,
    Disabled,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EtlMode {
    Passthrough,
    Vector,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Victoriametrics,
    Mysql,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataSource {
    pub datasource_id: String,
    pub data_link_id: String,
    pub producer: String,
    pub data_type: DataType,
    pub collect_method: CollectMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    pub dimension_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_ref: Option<String>,
    #[serde(default)]
    pub config: StringMap,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EtlPipeline {
    pub etl_pipeline_id: String,
    pub data_link_id: String,
    pub mode: EtlMode,
    #[serde(default)]
    pub config: StringMap,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultTable {
    pub result_table_id: String,
    pub data_link_id: String,
    pub result_table_name: String,
    pub storage_type: StorageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_cluster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_template: Option<String>,
    #[serde(default)]
    pub schema: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataLink {
    pub data_link_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub domain: String,
    pub owner_service: String,
    pub data_type: DataType,
    pub datasource_id: String,
    pub etl_pipeline_id: String,
    pub result_table_id: String,
    pub result_table_name: String,
    pub status: DataLinkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataLinkBundle {
    pub data_link: DataLink,
    pub datasource: DataSource,
    pub etl_pipeline: EtlPipeline,
    pub result_table: ResultTable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyDataLinkSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub domain: String,
    pub owner_service: String,
    pub data_type: DataType,
    pub status: DataLinkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub datasource: DataSourceInput,
    pub etl_pipeline: EtlPipelineInput,
    pub result_table: ResultTableInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataSourceInput {
    pub producer: String,
    pub data_type: DataType,
    pub collect_method: CollectMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    pub dimension_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_ref: Option<String>,
    #[serde(default)]
    pub config: StringMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EtlPipelineInput {
    pub mode: EtlMode,
    #[serde(default)]
    pub config: StringMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultTableInput {
    pub result_table_name: String,
    pub storage_type: StorageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_cluster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_template: Option<String>,
    #[serde(default)]
    pub schema: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetDataLinkStatus {
    pub status: DataLinkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationParams {
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataLinkListFilter {
    pub domain: Option<String>,
    pub owner_service: Option<String>,
    pub data_type: Option<DataType>,
    pub status: Option<DataLinkStatus>,
    pub storage_type: Option<StorageType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
```

---

## 2. error.rs 草案

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataLinkError {
    InvalidArgument(String),
    EnumNotSupported(String),
    ResultTableNameConflict(String),
    EtlPipelineInvalid(String),
    EtlModeNotSupported(String),
    NotFound(String),
    StatusTransitionInvalid { from: String, to: String },
    StatusMessageRequired,
    StatusMessageInvalid(String),
    IdempotencyConflict(String),
    BackendNotSupported(String),
    Repository(String),
}

impl fmt::Display for DataLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DataLinkError {}

pub type Result<T> = std::result::Result<T, DataLinkError>;
```

---

## 3. repository.rs 草案

```rust
use async_trait::async_trait;

use crate::{
    ApplyDataLinkSpec, DataLinkBundle, DataLinkListFilter, DataLinkStatus, PaginatedResult,
    PaginationParams, Result,
};

#[async_trait]
pub trait DataLinkRepository: Clone + Send + Sync + 'static {
    async fn apply(&self, spec: ApplyDataLinkSpec) -> Result<DataLinkBundle>;
    async fn get_by_id(&self, data_link_id: &str) -> Result<Option<DataLinkBundle>>;
    async fn get_by_result_table_name(&self, result_table_name: &str)
        -> Result<Option<DataLinkBundle>>;
    async fn list(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>>;
    async fn set_status(
        &self,
        data_link_id: &str,
        status: DataLinkStatus,
        status_message: Option<String>,
    ) -> Result<DataLinkBundle>;
}
```

---

## 4. service.rs 对外方法草案

```rust
#[derive(Debug, Clone)]
pub struct DataLinkService<R>
where
    R: DataLinkRepository,
{
    repository: R,
}

impl<R> DataLinkService<R>
where
    R: DataLinkRepository,
{
    pub fn new(repository: R) -> Self { ... }

    pub async fn apply_data_link(&self, spec: ApplyDataLinkSpec) -> Result<DataLinkBundle> { ... }

    pub async fn get_data_link(&self, data_link_id: &str) -> Result<Option<DataLinkBundle>> { ... }

    pub async fn get_data_link_by_result_table_name(
        &self,
        result_table_name: &str,
    ) -> Result<Option<DataLinkBundle>> { ... }

    pub async fn list_data_links(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>> { ... }

    pub async fn set_data_link_status(
        &self,
        data_link_id: &str,
        request: SetDataLinkStatus,
    ) -> Result<DataLinkBundle> { ... }
}
```

---

## 5. service 校验规则（必须实现）

### 5.1 ETL 规则

- `etl_pipeline` 必填。
- `mode` 只允许 `passthrough` / `vector`。
- `mode = passthrough`：`config` 可空。
- `mode = vector`：`config` 建议非空（至少包含一个键值对）。

### 5.2 状态规则

- `active -> status_message = None`
- `disabled -> status_message` 必须非空（或 `reason` 映射后非空）
- `draft/deleted -> status_message` 可选
- 状态流转：
  - `draft -> active`
  - `active -> disabled`
  - `disabled -> active`
  - `draft|active|disabled -> deleted`
  - 禁止 `deleted -> active`

### 5.3 唯一性规则

- `result_table_name` 全局唯一。

---

## 6. 协议映射建议（apiserver）

`apiserver/src/datalink_engine.rs` 建议只做：

- JSON DTO -> `ApplyDataLinkSpec`
- `DataLinkBundle` -> JSON Response
- `DataLinkError` -> HTTP status + error code

不要在 route 层写状态流转和 ETL 规则校验，全部放 `service`。

---

## 7. 落地顺序建议

1. 先落 `models.rs` + `error.rs`。
2. 落 `repository.rs` trait + `storage/memory.rs`。
3. 落 `service.rs`（把规则写全）。
4. 接入 `apiserver/src/datalink_engine.rs`。
5. 再补 `storage/mysql.rs`。

这样可以先把领域行为跑通，再做持久化优化。
