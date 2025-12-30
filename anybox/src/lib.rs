//! Anybox - 类似 paste.ubuntu 的文本分享服务
//!
//! ## 功能特性
//! - 文本帖子（TextBox）管理
//! - 多种文本格式支持
//! - 元数据管理
//! - 分页列表
//! - Redis 存储
//! - 过期自动清理
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use anybox::{RedisConfig, TextBoxManager, TextBox};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 创建管理器
//!     let config = RedisConfig::new("redis://127.0.0.1:6379".to_string());
//!     let mut manager = TextBoxManager::new(config).await?;
//!
//!     // 创建文本帖子
//!     let text_box = TextBox::new("Alice".to_string(), "Hello, world!".to_string())
//!         .with_title("My First Post".to_string());
//!
//!     let created = manager.create(text_box).await?;
//!     println!("Created: {}", created.id);
//!
//!     // 获取帖子
//!     if let Some(fetched) = manager.get(&created.id).await? {
//!         println!("Content: {}", fetched.content);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod models;
pub mod storage;

// 重新导出常用类型
pub use models::{PaginatedResult, PaginationParams, TextBox, TextBoxMetadata, TextFormat};
pub use storage::{RedisConfig, TextBoxManager, TextBoxStats};
