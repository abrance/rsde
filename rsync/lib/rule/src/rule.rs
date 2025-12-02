use crate::event::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// 自定义错误类型
#[derive(Debug, Clone)]
pub enum RsyncError {
    BuildError(String),
    ReadError(String),
    WriteError(String),
    TransformError(String),
    ConfigError(String),
}

impl fmt::Display for RsyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RsyncError::BuildError(msg) => write!(f, "Build error: {msg}"),
            RsyncError::ReadError(msg) => write!(f, "Read error: {msg}"),
            RsyncError::WriteError(msg) => write!(f, "Write error: {msg}"),
            RsyncError::TransformError(msg) => write!(f, "Transform error: {msg}"),
            RsyncError::ConfigError(msg) => write!(f, "Config error: {msg}"),
        }
    }
}

impl std::error::Error for RsyncError {}

// 从标准库 I/O 错误转换
impl From<std::io::Error> for RsyncError {
    fn from(err: std::io::Error) -> Self {
        RsyncError::WriteError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RsyncError>;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize, Serialize)]
pub struct ComponentKey<T = String> {
    pub id: T,
}

impl<T> ComponentKey<T> {
    pub fn id(&self) -> &T {
        &self.id
    }
}

/// 默认情况下，`ComponentKey` 使用 `String` 作为内部 ID`，
/// 因此既可以存放普通字符串，也可以存放 UUID 的字符串表示。
impl From<String> for ComponentKey<String> {
    fn from(value: String) -> Self {
        Self { id: value }
    }
}

impl From<&str> for ComponentKey<String> {
    fn from(value: &str) -> Self {
        Self {
            id: value.to_owned(),
        }
    }
}

/// 直接使用 `Uuid` 作为组件 ID。
///
/// `uuid::Uuid` 在启用 `serde` feature 后，序列化/反序列化默认就是
/// 字符串形式（例如 `"550e8400-e29b-41d4-a716-446655440000"`），
/// 满足“保存为 string”的需求。
impl From<Uuid> for ComponentKey<Uuid> {
    fn from(value: Uuid) -> Self {
        Self { id: value }
    }
}

impl From<&Uuid> for ComponentKey<Uuid> {
    fn from(value: &Uuid) -> Self {
        Self { id: *value }
    }
}

impl<T> fmt::Display for ComponentKey<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

/// 数据源输出配置
#[derive(Debug, Clone)]
pub struct SourceOutput {
    pub output_id: String,
    pub event_type: EventType,
}

/// 数据源上下文，包含构建和运行时所需的配置
pub struct SourceContext {
    pub key: ComponentKey,
    pub acknowledgements: bool,
    // 未来可扩展：
    // pub shutdown_signal: ShutdownSignal,
    // pub metrics: MetricsCollector,
}

/// Transform 上下文
pub struct TransformContext {
    pub key: ComponentKey,
}

/// Sink 上下文
pub struct SinkContext {
    pub key: ComponentKey,
    pub acknowledgements: bool,
}

/// Source trait - 数据源抽象
/// 负责从外部系统读取数据并转换为内部事件
#[async_trait]
#[typetag::serde(tag = "source_type")]
pub trait Source: Send + Sync + fmt::Debug {
    fn clone_box(&self) -> Box<dyn Source>;

    /// 获取数据源的输出配置列表
    /// 一个数据源可以有多个输出（例如：文件源可能输出多种类型的文件）
    fn outputs(&self) -> Vec<SourceOutput>;

    /// 构建并启动数据源
    /// 返回一个运行中的数据源句柄
    async fn build(&self, cx: SourceContext) -> Result<Box<dyn SourceRuntime>>;

    /// 指示数据源是否支持确认机制
    /// 例如：Kafka 消费者可以 ACK 消息，文件源则不需要
    fn can_acknowledge(&self) -> bool {
        false
    }

    /// 获取数据源类型的描述性名称
    fn source_type(&self) -> &str;
}

impl Clone for Box<dyn Source> {
    fn clone(&self) -> Box<dyn Source> {
        self.clone_box()
    }
}

/// 运行时的数据源实例
/// 将配置(Source)与运行时状态分离
#[async_trait]
pub trait SourceRuntime: Send + Sync {
    /// 从数据源读取下一个事件
    /// 返回 None 表示数据源已耗尽（如文件读完）
    async fn next_event(&mut self) -> Result<Option<Box<dyn Event>>>;

    /// 确认事件已被成功处理（仅当 can_acknowledge 为 true 时有效）
    async fn acknowledge(&mut self, _event_id: &str) -> Result<()> {
        Ok(())
    }

    /// 优雅关闭数据源
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Transform trait - 数据转换抽象
/// 负责对事件进行转换、过滤、富化等操作
#[async_trait]
#[typetag::serde(tag = "transform_type")]
pub trait Transform: Send + Sync + fmt::Debug {
    fn clone_box(&self) -> Box<dyn Transform>;

    /// 构建转换器实例
    async fn build(&self, cx: TransformContext) -> Result<Box<dyn TransformRuntime>>;

    /// 获取转换器类型名称
    fn transform_type(&self) -> &str;
}

impl Clone for Box<dyn Transform> {
    fn clone(&self) -> Box<dyn Transform> {
        self.clone_box()
    }
}

/// 运行时的转换器实例
#[async_trait]
pub trait TransformRuntime: Send + Sync {
    /// 处理单个事件
    /// 返回转换后的事件，可能返回 None（过滤掉事件）
    /// 或返回多个事件（一对多转换）
    async fn process(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Event>>>;
}

/// Sink trait - 数据目的地抽象
/// 负责将事件写入到外部系统
#[async_trait]
#[typetag::serde(tag = "sink_type")]
pub trait Sink: Send + Sync + fmt::Debug {
    fn clone_box(&self) -> Box<dyn Sink>;

    /// 构建 Sink 实例
    async fn build(&self, cx: SinkContext) -> Result<Box<dyn SinkRuntime>>;

    /// 获取 Sink 类型名称
    fn sink_type(&self) -> &str;
}

impl Clone for Box<dyn Sink> {
    fn clone(&self) -> Box<dyn Sink> {
        self.clone_box()
    }
}

/// 运行时的 Sink 实例
#[async_trait]
pub trait SinkRuntime: Send + Sync {
    /// 写入单个事件
    async fn write(&mut self, event: Box<dyn Event>) -> Result<()>;

    /// 批量写入事件（可选优化）
    async fn write_batch(&mut self, events: Vec<Box<dyn Event>>) -> Result<()> {
        for event in events {
            self.write(event).await?;
        }
        Ok(())
    }

    /// 刷新缓冲区，确保数据被写入
    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    /// 优雅关闭
    async fn shutdown(&mut self) -> Result<()> {
        self.flush().await
    }
}

/// 数据传输管道的元数据
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataTransferMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// 数据传输管道配置
/// 定义了一个完整的 Source -> Transform -> Sink 流程
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataTransferConfig {
    pub metadata: DataTransferMetadata,
    /// 数据源配置列表（序列化存储）
    #[serde(default)]
    pub sources: Vec<Box<dyn Source>>,
    /// 转换器配置列表
    #[serde(default)]
    pub transforms: Vec<Box<dyn Transform>>,
    /// 目标配置列表
    #[serde(default)]
    pub sinks: Vec<Box<dyn Sink>>,
}

impl DataTransferConfig {
    pub fn new(
        id: String,
        name: String,
        description: Option<String>,
        sources: Vec<Box<dyn Source>>,
        transforms: Vec<Box<dyn Transform>>,
        sinks: Vec<Box<dyn Sink>>,
    ) -> Self {
        Self {
            metadata: DataTransferMetadata {
                id,
                name,
                description,
            },
            sources,
            transforms,
            sinks,
        }
    }
}
