pub mod controller;
pub mod event;
pub mod file;
pub mod rule_file_watch;

/// Rule 模块定义了 rsync 的核心抽象：Source, Transform, Sink
///
/// - Source: 数据源，负责读取数据
/// - Transform: 数据转换器，负责转换和处理数据
/// - Sink: 数据目的地，负责写入数据
///
/// 详细设计文档请参考 DESIGN.md
pub mod rule;

// 重新导出常用类型
pub use rule::{
    ComponentKey, DataTransferConfig, DataTransferMetadata, Result, RsyncError, Sink, SinkContext,
    SinkRuntime, Source, SourceContext, SourceOutput, SourceRuntime, Transform, TransformContext,
    TransformRuntime,
};

// 导出平台相关类型
pub use file::*;
