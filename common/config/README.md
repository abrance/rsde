# 统一配置管理

本项目使用 `common/config` 库统一管理所有服务的配置定义。

## 目录结构

```
common/config/
├── src/
│   ├── lib.rs              # 主模块，定义 GlobalConfig
│   ├── ocr.rs              # OCR 相关配置
│   ├── apiserver.rs        # API Server 配置
│   ├── object_storage.rs   # 七牛云对象存储配置
│   ├── nodemanage.rs       # NodeManage 节点管理配置
│   └── rsync.rs            # Rsync 服务配置
└── Cargo.toml
```

## 依赖关系

```
common/config (基础配置库，不依赖其他模块)
    ↑
    ├── pic_recog (使用 config::ocr::*)
    ├── apiserver (使用 config::GlobalConfig, config::ocr::*)
    ├── nodemanage (通过 apiserver 读取 config::nodemanage::NodeManageConfig)
    ├── rsync (使用 config::rsync::*)
    └── rc (使用 config::*)
```

## 使用方法

### 1. 在 Cargo.toml 中添加依赖

```toml
[dependencies]
config = { path = "../common/config" }
```

### 2. 加载配置

```rust
use config::{ConfigLoader, GlobalConfig};

// 加载全局配置文件
let config = GlobalConfig::from_file("config.toml")?;

// 访问特定服务的配置
if let Some(ocr_config) = config.remote_ocr {
    // 使用 OCR 配置
}

if let Some(apiserver_config) = config.apiserver {
    // 使用 API Server 配置
}

if let Some(nodemanage_config) = config.nodemanage {
    // 使用 NodeManage 配置
}
```

### 3. 使用配置结构体

```rust
// 使用 OCR 配置
use config::ocr::{OcrConfig, RemoteOcrConfig};

let ocr_config = OcrConfig::new()
    .with_language("chi_sim")
    .with_psm(6);

// 使用 Rsync 配置
use config::rsync::RsyncConfig;

let rsync_config = RsyncConfig::from_file("rsync_config.toml")?;
```

## 配置文件格式

项目支持单个统一配置文件，包含所有服务的配置：

```toml
# config.toml

[apiserver]
listen_address = "0.0.0.0:3000"
log_level = "info"

[remote_ocr]
perm_url = "https://example.com/api/perm"
# ... 其他 OCR 配置

[rsync]
# ... Rsync 配置

[nodemanage]
table_prefix = "node_"
rsagent_package_url = "https://example.com/rsagent.tar.gz"
install_root = "/opt/rsagent"
register_callback_url = "http://127.0.0.1:3000/api/nodes/agent/register"
register_wait_timeout_secs = 30
ssh_connect_timeout_secs = 10

[[nodemanage.install_plugins]]
name = "metrics"
version = "1.0.0"
package_url = "https://example.com/plugins/metrics.tar.gz"

[nodemanage.mysql]
host = "127.0.0.1"
port = 3306
user = "root"
password = "your_mysql_password_here"
database = "rsde"
max_connections = 10
min_connections = 1
connect_timeout_secs = 10
```

参考 `config.example.toml` 获取完整示例。

### 对象存储配置说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `access_key` | 是 | 七牛云 Access Key |
| `secret_key` | 是 | 七牛云 Secret Key |
| `bucket` | 是 | 对象存储 bucket 名称 |
| `region` | 是 | 七牛云区域标识，如 `z0`、`z1`、`z2`、`na0`、`as0` |
| `domain` | 否 | bucket 访问域名 |
| `public_base_url` | 否 | 公开访问场景下的显式 URL 前缀 |
| `upload_token_ttl_secs` | 否 | 上传 token 过期时间（秒），默认 3600 |
| `private_url_ttl_secs` | 否 | 私有下载链接过期时间（秒），默认 3600 |
| `use_https` | 否 | 生成 URL 时是否优先使用 HTTPS，默认 true |
| `path_prefix` | 否 | 部署级别的逻辑前缀约束 |
| `bucket_is_private` | 否 | bucket 是否为私有，默认 false |

**配置规则：**

- 如果 `[object_storage]` 不存在，则对象存储功能关闭
- 必填字段（`access_key`、`secret_key`、`bucket`、`region`）不能为空或仅包含空白字符，否则启动失败
- `region` 必须是允许的值之一：`z0`、`z1`、`z2`、`na0`、`as0`，否则启动失败
- 当 `bucket_is_private = false` 时，`domain` 或 `public_base_url` 至少要配置一个且非空字符串（不能仅为空白字符）
- `path_prefix` 自动规范化：去除前后空白、移除前导 `/`、非空时添加尾部 `/`；若规范化后为空则返回 `None`
- 配置验证在 `GlobalConfig::from_file()` 加载时自动执行，无效配置会导致启动失败

### NodeManage 配置说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `table_prefix` | 否 | 节点管理表名前缀，默认 `node_`，后续 MySQL 持久化使用。 |
| `rsagent_package_url` | 否 | 在线添加节点时安装的 rsagent 包地址；也可由 `/api/nodes/install` 请求体覆盖。 |
| `install_root` | 否 | rsagent 默认安装目录，默认 `/opt/rsagent`。 |
| `register_callback_url` | 否 | 远端 rsagent 安装完成后的回调注册地址。 |
| `install_plugins` | 否 | 安装时默认需要下发的插件列表。 |
| `register_wait_timeout_secs` | 否 | 等待 agent 注册成功的超时时间，默认 `30` 秒。 |
| `ssh_connect_timeout_secs` | 否 | SSH 连接超时时间，单位秒，默认 `10`。 |
| `[nodemanage.mysql]` | 否 | NodeManage 独立 MySQL 配置；未配置时当前实现使用内存仓储。 |

## 设计优势

1. **统一管理**：所有配置定义集中在 `common/config`
2. **避免重复**：不同服务共享配置结构，避免重复定义
3. **单一配置文件**：一个 TOML 文件管理所有服务配置
4. **类型安全**：使用 Rust 类型系统保证配置正确性
5. **易于维护**：配置变更只需修改一处
