# 识别引擎模块

本目录包含不同的图片识别引擎实现。

## 已实现的引擎

### Tesseract OCR
- 位置: `tesseract/`
- 说明: 基于 Tesseract OCR 的开源文字识别引擎
- 支持: 多语言识别、自定义配置、批量处理

## 如何添加新引擎

要添加新的识别引擎，请按照以下步骤：

### 1. 创建引擎目录

在 `engines/` 下创建新目录，例如 `paddleocr/`:

```
engines/
├── tesseract/
└── paddleocr/      # 新引擎
    ├── mod.rs
    ├── config.rs
    ├── recognizer.rs
    └── utils.rs
```

### 2. 实现核心模块

#### mod.rs
```rust
//! PaddleOCR 引擎

pub mod config;
pub mod recognizer;
pub mod utils;

pub use config::PaddleConfig;
pub use recognizer::{recognize, recognize_batch};
```

#### config.rs
```rust
//! PaddleOCR 配置

use crate::config::OcrConfig;

pub struct PaddleConfig {
    pub base: OcrConfig,
    // PaddleOCR 特定配置
    pub det_model_path: Option<String>,
    pub rec_model_path: Option<String>,
}
```

#### recognizer.rs
```rust
//! PaddleOCR 识别器

use crate::config::OcrConfig;
use crate::error::ImageRecognitionError;
use crate::utils::validate_image_path;

pub fn recognize(
    image_path: &str,
    config: &OcrConfig,
) -> Result<String, ImageRecognitionError> {
    validate_image_path(image_path)?;
    
    // 实现具体的识别逻辑
    todo!("实现 PaddleOCR 识别")
}
```

### 3. 在 engines/mod.rs 中注册

```rust
pub mod tesseract;
pub mod paddleocr;  // 添加新引擎
```

### 4. 在 lib.rs 中添加便捷函数

```rust
/// 使用 PaddleOCR 识别图片
pub fn recognize_image_by_paddle(image_path: &str) -> Result<String, ImageRecognitionError> {
    let config = OcrConfig::default();
    engines::paddleocr::recognize(image_path, &config)
}
```

### 5. 更新依赖（如需要）

在 `Cargo.toml` 中添加新引擎的依赖:

```toml
[dependencies]
paddleocr = { version = "0.1", optional = true }

[features]
default = ["tesseract"]
tesseract = ["tesseract-rs"]
paddle = ["paddleocr"]
```

## 引擎设计原则

1. **统一接口**: 所有引擎都应提供 `recognize` 和 `recognize_batch` 函数
2. **配置分离**: 引擎特定配置放在各自的 `config.rs` 中
3. **错误处理**: 使用统一的 `ImageRecognitionError` 类型
4. **可选依赖**: 通过 feature flag 控制引擎的启用

## 可以考虑添加的引擎

- **PaddleOCR**: 百度开源的 OCR 引擎，支持中文
- **EasyOCR**: 轻量级 OCR 引擎，支持 80+ 语言
- **Azure Computer Vision**: 微软云服务
- **Google Cloud Vision**: 谷歌云服务
- **AWS Textract**: 亚马逊云服务
- **TrOCR**: 基于 Transformer 的 OCR 模型

## 参考资源

- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)
- [EasyOCR](https://github.com/JaidedAI/EasyOCR)
