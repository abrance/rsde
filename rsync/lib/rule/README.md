# Rule - Rsync 规则库

## 概述

这是 rsync 项目的核心规则库，定义了数据同步的抽象接口和类型系统。

## 核心概念

### Source → Transform → Sink 管道模式

```
[数据源] --Event--> [转换器] --Event--> [目的地]
 Source              Transform            Sink
```

每个组件分为两层：
- **配置层** (Source/Transform/Sink): 可序列化的配置，描述"做什么"
- **运行时层** (SourceRuntime/TransformRuntime/SinkRuntime): 实际执行逻辑，描述"怎么做"

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
rule = { path = "./lib/rule" }
tokio = { version = "1.0", features = ["full"] }
```

### 2. 实现一个简单的文件源

```rust
use rule::*;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct MyFileSource {
    path: String,
}

#[typetag::serde]
#[async_trait]
impl Source for MyFileSource {
    fn outputs(&self) -> Vec<SourceOutput> {
        vec![SourceOutput {
            output_id: "default".to_string(),
            event_type: EventType::Text(TextType::PlainText),
        }]
    }

    async fn build(&self, _cx: SourceContext) -> Result<Box<dyn SourceRuntime>> {
        // 实现构建逻辑
        todo!()
    }

    fn source_type(&self) -> &str {
        "file"
    }
}
```

### 3. 查看完整示例

详细的实现示例请参考 `src/examples.rs`

## 主要类型

### Event (事件)

所有数据的统一表示：

```rust
pub trait Event: Send + Sync {
    fn get_metadata(&self) -> &EventMetadata;
    fn get_payload(&self) -> &Vec<u8>;
}
```

### Source (数据源)

从外部系统读取数据：

```rust
#[async_trait]
pub trait Source {
    fn outputs(&self) -> Vec<SourceOutput>;
    async fn build(&self, cx: SourceContext) -> Result<Box<dyn SourceRuntime>>;
    fn source_type(&self) -> &str;
}

#[async_trait]
pub trait SourceRuntime {
    async fn next_event(&mut self) -> Result<Option<Box<dyn Event>>>;
    async fn shutdown(&mut self) -> Result<()>;
}
```

### Transform (转换器)

转换、过滤、富化数据：

```rust
#[async_trait]
pub trait Transform {
    async fn build(&self, cx: TransformContext) -> Result<Box<dyn TransformRuntime>>;
    fn transform_type(&self) -> &str;
}

#[async_trait]
pub trait TransformRuntime {
    async fn process(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Event>>>;
}
```

### Sink (目的地)

将数据写入外部系统：

```rust
#[async_trait]
pub trait Sink {
    async fn build(&self, cx: SinkContext) -> Result<Box<dyn SinkRuntime>>;
    fn sink_type(&self) -> &str;
}

#[async_trait]
pub trait SinkRuntime {
    async fn write(&mut self, event: Box<dyn Event>) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
}
```

## 特性

✅ **类型安全**: 完全利用 Rust 类型系统  
✅ **异步优先**: 所有 I/O 操作都是异步的  
✅ **可序列化**: 配置可以保存和加载  
✅ **模块化**: 组件完全解耦，可独立开发  
✅ **可扩展**: 通过 trait 轻松添加新组件  
✅ **线程安全**: `Send + Sync` 保证并发安全  

## 文档

- [DESIGN.md](./DESIGN.md) - 详细设计文档
- [src/examples.rs](./src/examples.rs) - 完整的使用示例
- [src/rule.rs](./src/rule.rs) - 核心 trait 定义

## 设计优势

相比原始设计，新版本解决了以下问题：

| 问题 | 解决方案 |
|------|---------|
| build 返回类型错误 | 返回 `Box<dyn Runtime>` |
| 缺少运行时状态 | 分离配置和运行时 |
| 同步 I/O | 全面异步化 |
| Transform 只能一对一 | 支持一对多、过滤 |
| 缺少批量优化 | 添加 `write_batch` |
| 错误类型不明确 | 自定义 `RsyncError` |
| 生命周期管理不清晰 | 添加 `shutdown` 方法 |

## 测试

```bash
cargo test
```

## 许可

[根据项目许可]
