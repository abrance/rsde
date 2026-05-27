# datalink-engine 代码目录设计

## 1. 设计目标

`datalink-engine` 应该作为一个独立的 workspace crate 存在，而不是先塞进 `apiserver` 再拆分。

原因很明确：

- 当前仓库已经有 `nodemanage` 这种“独立领域 crate + apiserver 路由集成”的模式。
- `datalink-engine` 本质上也是一个独立领域服务，后续会被 `nodemanage`、`query-engine`、`job-manage` 等多个组件共同依赖。
- 如果一开始就放进 `apiserver`，后续再拆会把 domain/service/repository/router 边界重新切一遍，成本更高。

因此推荐采用：

> **独立 crate 承载领域模型与核心逻辑，`apiserver` 仅负责 HTTP 路由和进程级装配。**

---

## 2. 推荐目录形态

### 2.1 workspace 根目录

在 workspace 根下新增：

```text
rsde/
├── datalink-engine/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── bootstrap.rs
│   │   ├── error.rs
│   │   ├── models.rs
│   │   ├── repository.rs
│   │   ├── service.rs
│   │   ├── protocol.rs
│   │   └── storage/
│   │       ├── mod.rs
│   │       ├── memory.rs
│   │       └── mysql.rs
│   └── tests/
│       ├── domain.rs
│       ├── service.rs
│       └── repository.rs
```

同时在根 `Cargo.toml` 中把它加入 workspace members。

---

## 3. 各目录职责

### 3.1 `src/lib.rs`

crate 对外暴露入口。

建议负责：

- 导出公共模块；
- 统一导出关键类型，如 `DataLinkService`、`DataLinkRepository`、`DataLinkError`；
- 保持简洁，不堆业务逻辑。

---

### 3.2 `src/models.rs`

承载领域模型和核心数据结构。

建议放：

- `DataLink`
- `DataSource`
- `EtlPipeline`
- `ResultTable`
- `DataLinkStatus`
- `ApplyDataLinkSpec`
- `SetDataLinkStatusRequest`

这里的重点是：

- `models.rs` 放的是**领域对象**；
- 不要把 HTTP request/response DTO 和数据库行模型全部混在一起。

如果后续字段继续膨胀，可以再拆成：

```text
src/models/
├── mod.rs
├── data_link.rs
├── datasource.rs
├── etl_pipeline.rs
└── result_table.rs
```

但第一阶段先单文件即可，避免过早碎片化。

---

### 3.3 `src/error.rs`

统一错误定义。

建议包含：

- 参数错误
- 状态流转错误
- `result_table_name` 冲突
- `status_message` 校验错误
- ETL 定义错误
- 仓储层错误

目标是让 service、repository、apiserver route 都能共享同一套语义错误。

---

### 3.4 `src/repository.rs`

定义仓储抽象接口，不放具体数据库实现。

例如：

- `DataLinkRepository`
- `DataSourceRepository`
- `EtlPipelineRepository`
- `ResultTableRepository`

或者在第一阶段先用一个聚合接口：

- `DataLinkRepository`

统一承载：

- apply/upsert
- get by id
- get by result_table_name
- list
- set status

推荐第一阶段用**聚合仓储接口**，因为业务动作天然是围绕整条 datalink 展开，而不是四张表完全独立 CRUD。

---

### 3.5 `src/service.rs`

承载核心业务逻辑，是整个 crate 的重心。

建议在这里实现：

- `apply_data_link`
- `get_data_link`
- `get_data_link_by_result_table_name`
- `list_data_links`
- `set_data_link_status`
- 领域校验逻辑

包括：

- `result_table_name` 全局唯一校验
- `etl_pipeline` 可选/必填字段校验
- `status_message` 生命周期规则
- `status` 状态流转规则
- direct-write vs ETL-managed 链路规则

原则：

- repository 只负责存取；
- service 负责业务规则；
- route 层不做业务判断。

---

### 3.6 `src/protocol.rs`

承载对外协议对象定义，主要服务于 `apiserver` 的 HTTP API。

建议放：

- `ApplyDataLinkRequest`
- `ApplyDataLinkResponse`
- `GetDataLinkResponse`
- `ListDataLinksResponse`
- `SetDataLinkStatusRequestDto`
- 错误响应 DTO

这样可以把：

- 领域模型（`models.rs`）
- 外部协议模型（`protocol.rs`）

分开，避免后续 API 变动直接污染领域对象。

---

### 3.7 `src/bootstrap.rs`

负责组装 service 和 repository 实现。

例如：

- `build_memory_service()`
- `build_mysql_service()`

或者定义：

- `DataLinkModule`

用于集中初始化：

- repository
- service
- 配置项
- metrics 注册

这个文件的目标是避免 `main.rs` 或 route 模块里写太多装配代码。

---

### 3.8 `src/storage/`

放具体存储实现。

推荐结构：

```text
src/storage/
├── mod.rs
├── memory.rs
└── mysql.rs
```

职责：

- `memory.rs`：第一阶段开发/测试用内存实现；
- `mysql.rs`：后续正式持久化实现；

如果后续复杂度上升，再拆成：

```text
src/storage/mysql/
├── mod.rs
├── data_link.rs
├── datasource.rs
├── etl_pipeline.rs
└── result_table.rs
```

第一阶段不建议一开始就拆这么细，因为当前核心是把业务边界立住，不是先做 ORM 文件铺满。

---

## 4. 与 apiserver 的关系

`apiserver` 不应该承载 `datalink-engine` 的核心逻辑，只应该做集成入口。

建议在 `apiserver` 中增加：

```text
apiserver/
└── src/
    └── datalink_engine.rs
```

职责仅包括：

- 注册 HTTP routes
- 请求/响应转换
- 调用 `datalink_engine::service`
- 将领域错误映射成 HTTP 错误码

也就是说：

- `datalink-engine/` 是领域服务实现；
- `apiserver/src/datalink_engine.rs` 是 HTTP 适配层。

这和当前：

- `nodemanage/` 负责领域逻辑
- `apiserver/src/nodemanage.rs` 负责路由集成

的模式保持一致。

---

## 5. 第一阶段推荐最小实现

为了避免一开始目录过重，第一阶段建议最小可行结构如下：

```text
datalink-engine/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── bootstrap.rs
│   ├── error.rs
│   ├── models.rs
│   ├── protocol.rs
│   ├── repository.rs
│   ├── service.rs
│   └── storage/
│       ├── mod.rs
│       ├── memory.rs
│       └── mysql.rs
└── tests/
    ├── domain.rs
    ├── service.rs
    └── repository.rs
```

这是当前最平衡的方案：

- 比全部塞进 `apiserver` 清晰；
- 比一开始就 `application/domain/infrastructure/interfaces` 四层大拆分更轻；
- 足够支撑后续扩展到 query-engine / job-manage / nodemanage 共同依赖。

---

## 6. 三种组织方式对比

### 方案 A：全部放进 `apiserver`

例如：

```text
apiserver/src/datalink_engine/
```

优点：

- 上手快；
- 改动路径短。

缺点：

- 领域逻辑和 HTTP 适配耦合；
- 后续独立服务化时拆分成本高；
- 不符合当前 `nodemanage` 的仓库模式。

**不推荐。**

---

### 方案 B：独立 crate + apiserver 集成（推荐）

例如：

```text
datalink-engine/
apiserver/src/datalink_engine.rs
```

优点：

- 领域边界清晰；
- 易测试；
- 易独立部署；
- 与 `nodemanage` 模式一致。

缺点：

- 初期文件数稍多；
- 需要做一层 protocol/route 映射。

**推荐采用。**

---

### 方案 C：独立 crate，但内部直接分层过深

例如：

```text
src/
├── application/
├── domain/
├── infrastructure/
├── interfaces/
└── adapters/
```

优点：

- 理论分层最完整；
- 对超大项目扩展友好。

缺点：

- 第一阶段过度设计；
- 当前仓库没有统一采用这套风格；
- 会显著增加理解和维护成本。

**当前阶段不推荐。**

---

## 7. 推荐结论

推荐结论只有一句话：

> **新增顶层 crate：`datalink-engine/`，内部采用“models + repository + service + protocol + storage + bootstrap”的轻量领域分层；`apiserver` 只保留 `datalink_engine.rs` 作为 HTTP 集成入口。**

这套组织方式最符合当前仓库已有风格，也最适合未来演进成真正独立的数据链路服务。

---

## 8. 后续落地顺序建议

建议按以下顺序落地：

1. 在 workspace 中新增 `datalink-engine` crate。
2. 先实现 `models.rs`、`error.rs`、`repository.rs`、`service.rs`。
3. 先提供 `storage/memory.rs`，保证 API 和业务逻辑可跑通。
4. 在 `apiserver/src/datalink_engine.rs` 暴露 HTTP API。
5. 再补 `storage/mysql.rs` 做正式持久化。
6. 最后再根据规模决定是否把 `models.rs` / `storage/mysql.rs` 进一步拆细。

这样可以先把架子搭对，再逐步把实现填进去，而不是一开始就把目录拆得很复杂。
