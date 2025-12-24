# 统一配置管理

本项目使用 `common/config` 库统一管理所有服务的配置定义。

## 目录结构

```
common/config/
├── src/
│   ├── lib.rs           # 主模块，定义 GlobalConfig
│   ├── ocr.rs           # OCR 相关配置
│   ├── apiserver.rs     # API Server 配置
│   └── rsync.rs         # Rsync 服务配置
└── Cargo.toml
```

## 依赖关系

```
common/config (基础配置库，不依赖其他模块)
    ↑
    ├── pic_recog (使用 config::ocr::*)
    ├── apiserver (使用 config::GlobalConfig, config::ocr::*)
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
```

参考 `config.example.toml` 获取完整示例。

## 优势

1. **统一管理**：所有配置定义集中在 `common/config`
2. **避免重复**：不同服务共享配置结构，避免重复定义
3. **单一配置文件**：一个 TOML 文件管理所有服务配置
4. **类型安全**：使用 Rust 类型系统保证配置正确性
5. **易于维护**：配置变更只需修改一处
