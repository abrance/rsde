# datalink-engine

`datalink-engine` 是 rsde 中负责数据链路元数据登记、查询和状态管理的独立 workspace crate。

## 当前实现范围

本 crate 当前实现的是 V1 最小可用形态：

- `ApplyDataLink` 风格的声明式创建/更新
- 通过 `data_link_id` 查询链路详情
- 通过 `result_table_name` 查询链路详情
- 链路列表与过滤
- 链路状态更新
- 幂等 apply 语义

## V1 模型约束

- `etl_pipeline` 为必选对象
- `etl_pipeline.mode` 仅支持 `passthrough | vector`
- `etl_pipeline.config` 为 `map<string, string>`
- V1 不接受旧字段，例如 `metadata`、`steps`、`pipeline_name`、`version`、`enabled`

## 模块职责

- `src/models.rs`：领域模型、输入模型、分页与过滤参数
- `src/error.rs`：V1 领域错误
- `src/repository.rs`：仓储抽象
- `src/storage/memory.rs`：内存后端实现
- `src/storage/mysql.rs`：MySQL 占位实现
- `src/service.rs`：校验、幂等、状态流转、查询逻辑
- `src/bootstrap.rs`：runtime service 构建入口

## 后端状态

- `memory`：已实现，可用于本地开发与接口测试
- `mysql`：配置入口已预留，但当前仍返回 `BackendNotSupported`

## apiserver 集成

`apiserver` 在配置存在时挂载 `/api/datalink/v1` 路由。启用示例见仓库根目录的 `config.example.toml`：

```toml
[datalink_engine]
backend = "memory"
```

## 验证命令

```bash
cargo test -p datalink-engine
cargo check -p datalink-engine
cargo test -p apiserver --test datalink_engine_routes
```
