# Rsync 规则系统设计文档

## 概述

rsync 规则系统采用 **Source → Transform → Sink** 的经典数据管道模式，用于定义和执行数据同步任务。

## 核心设计原则

### 1. 配置与运行时分离

```
Source (Config) → SourceRuntime (Instance)
Transform (Config) → TransformRuntime (Instance)
Sink (Config) → SinkRuntime (Instance)
```

**为什么这样设计？**

- **配置可序列化**: `Source/Transform/Sink` trait 可以通过 `typetag::serde` 序列化，方便存储和传输
- **运行时状态管理**: `Runtime` 版本管理实际的状态（如文件指针、网络连接、缓冲区等）
- **可重用性**: 同一个配置可以多次构建出不同的运行时实例

### 2. 异步优先设计

所有 I/O 操作都是异步的：
- `async fn build()` - 构建可能需要建立网络连接、打开文件等
- `async fn next_event()` - 读取可能是阻塞操作
- `async fn process()` - 转换可能需要调用外部 API
- `async fn write()` - 写入通常涉及 I/O

使用 `async_trait` 宏来支持 trait 中的异步方法。

### 3. 类型安全的事件系统

```rust
pub trait Event: Send + Sync {
    fn get_metadata(&self) -> &EventMetadata;
    fn get_payload(&self) -> &Vec<u8>;
}
```

- 所有数据统一为 `Event`，包含元数据和二进制载荷
- 通过 `EventType` 层次化地描述数据类型
- `Send + Sync` 保证线程安全

## 主要组件详解

### Source (数据源)

**职责**: 从外部系统读取数据并转换为内部事件

**配置方法**:
```rust
fn outputs(&self) -> Vec<SourceOutput>;  // 声明输出类型
fn can_acknowledge(&self) -> bool;        // 是否支持确认机制
fn source_type(&self) -> &str;            // 类型标识
async fn build(&self, cx: SourceContext) -> Result<Box<dyn SourceRuntime>>;
```

**运行时方法**:
```rust
async fn next_event(&mut self) -> Result<Option<Box<dyn Event>>>;
async fn acknowledge(&mut self, event_id: &str) -> Result<()>;
async fn shutdown(&mut self) -> Result<()>;
```

**典型实现**:
- 文件源: 读取文件内容
- Kafka 消费者: 订阅 topic 并消费消息
- HTTP API: 轮询或 webhook 接收数据
- 数据库: 查询或 CDC (Change Data Capture)

### Transform (数据转换)

**职责**: 对事件进行转换、过滤、富化等操作

**配置方法**:
```rust
fn transform_type(&self) -> &str;
async fn build(&self, cx: TransformContext) -> Result<Box<dyn TransformRuntime>>;
```

**运行时方法**:
```rust
async fn process(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Event>>>;
```

**返回 Vec 的设计意义**:
- 返回空 Vec: 过滤掉事件
- 返回单个事件: 一对一转换
- 返回多个事件: 一对多转换（如拆分、复制）

**典型实现**:
- JSON 解析/转换
- 字段映射
- 数据过滤
- 数据富化（查询外部数据并合并）
- 格式转换（CSV → JSON, XML → JSON 等）

### Sink (数据目的地)

**职责**: 将事件写入到外部系统

**配置方法**:
```rust
fn sink_type(&self) -> &str;
async fn build(&self, cx: SinkContext) -> Result<Box<dyn SinkRuntime>>;
```

**运行时方法**:
```rust
async fn write(&mut self, event: Box<dyn Event>) -> Result<()>;
async fn write_batch(&mut self, events: Vec<Box<dyn Event>>) -> Result<()>;
async fn flush(&mut self) -> Result<()>;
async fn shutdown(&mut self) -> Result<()>;
```

**批量写入优化**:
- `write_batch` 提供了批量写入优化的接口
- 默认实现是逐个调用 `write`，具体实现可以覆盖以获得更好的性能

**典型实现**:
- 文件写入
- 数据库插入
- HTTP POST
- 消息队列发布
- 对象存储（S3, OSS 等）

## 错误处理

自定义 `RsyncError` 枚举涵盖各类错误：
```rust
pub enum RsyncError {
    BuildError(String),      // 构建阶段错误
    ReadError(String),       // 读取错误
    WriteError(String),      // 写入错误
    TransformError(String),  // 转换错误
    ConfigError(String),     // 配置错误
}
```

统一的 `Result<T>` 类型使错误处理更加一致。

## 使用流程

### 1. 定义配置

```rust
let config = DataTransferConfig {
    metadata: DataTransferMetadata {
        id: "sync-1".to_string(),
        name: "File to HTTP".to_string(),
        description: Some("Sync files to HTTP endpoint".to_string()),
    },
    sources: vec![Box::new(FileSourceConfig { ... })],
    transforms: vec![Box::new(JsonTransformConfig { ... })],
    sinks: vec![Box::new(HttpSinkConfig { ... })],
};
```

### 2. 构建运行时

```rust
let mut source_runtime = config.sources[0].build(context).await?;
let mut transform_runtime = config.transforms[0].build(context).await?;
let mut sink_runtime = config.sinks[0].build(context).await?;
```

### 3. 执行数据流

```rust
while let Some(event) = source_runtime.next_event().await? {
    let transformed = transform_runtime.process(event).await?;
    for event in transformed {
        sink_runtime.write(event).await?;
    }
}
```

### 4. 清理资源

```rust
sink_runtime.shutdown().await?;
```

## 扩展性

### 添加新的数据源

1. 定义配置结构体（实现 `Serialize/Deserialize`）
2. 实现 `Source` trait
3. 定义运行时结构体
4. 实现 `SourceRuntime` trait
5. 添加 `#[typetag::serde]` 标注以支持序列化

### 添加新的转换器

同上，实现 `Transform` 和 `TransformRuntime`

### 添加新的 Sink

同上，实现 `Sink` 和 `SinkRuntime`

## 优势总结

1. **类型安全**: Rust 的类型系统保证编译时安全
2. **可序列化**: 配置可以保存为 JSON/YAML 等格式
3. **模块化**: Source/Transform/Sink 完全解耦，可独立开发和测试
4. **异步高效**: 充分利用 Rust 的异步特性，支持高并发
5. **可扩展**: 通过 trait 轻松添加新的组件类型
6. **灵活组合**: 支持多个 source/transform/sink 的任意组合

## 改进建议

原始设计存在的问题：

1. ❌ `build` 返回 trait 对象不可行 → ✅ 返回 `Box<dyn Runtime>`
2. ❌ 缺少运行时状态管理 → ✅ 分离配置和运行时
3. ❌ 同步设计不适合 I/O → ✅ 全面异步化
4. ❌ Transform 只能一对一 → ✅ 支持一对多、过滤
5. ❌ 缺少批量优化 → ✅ Sink 提供 `write_batch`
6. ❌ 错误类型不明确 → ✅ 自定义 `RsyncError`
7. ❌ 缺少生命周期管理 → ✅ 添加 `shutdown` 方法

## 下一步

1. 实现常用的 Source（文件、HTTP、Kafka）
2. 实现常用的 Transform（JSON、过滤、映射）
3. 实现常用的 Sink（文件、HTTP、数据库）
4. 添加监控和指标收集
5. 实现配置验证
6. 添加重试和错误恢复机制
7. 实现并行处理和背压控制
