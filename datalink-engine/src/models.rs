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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct EtlPipelineInput {
    pub mode: EtlMode,
    #[serde(default)]
    pub config: StringMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ApplyDataLinkOptions {
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SetDataLinkStatus {
    pub status: DataLinkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

impl PaginationParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> u64 {
        ((self.page - 1) * self.page_size) as u64
    }

    pub fn limit(&self) -> u64 {
        self.page_size as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, params: &PaginationParams) -> Self {
        let total_pages = ((total as f64) / (params.page_size as f64)).ceil() as u32;

        Self {
            items,
            total,
            page: params.page,
            page_size: params.page_size,
            total_pages: total_pages.max(1),
        }
    }
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
