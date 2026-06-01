use anyhow::{Context, Result};
use redis::{AsyncCommands, aio::ConnectionManager};
use std::fmt::Debug;
use tracing::{debug, info};

use crate::models::{PaginatedResult, PaginationParams, TextBox};

/// Redis 存储配置
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis 连接 URL
    pub url: String,
    /// 键前缀
    pub key_prefix: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            key_prefix: "anybox".to_string(),
        }
    }
}

impl RedisConfig {
    pub fn new(url: String) -> Self {
        Self {
            url,
            key_prefix: "anybox".to_string(),
        }
    }

    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.key_prefix = prefix;
        self
    }
}

/// TextBox 管理器
#[derive(Clone)]
pub struct TextBoxManager {
    /// Redis 连接管理器
    conn: ConnectionManager,
    /// 键前缀
    key_prefix: String,
}

impl TextBoxManager {
    /// 创建新的管理器
    pub async fn new(config: RedisConfig) -> Result<Self> {
        info!("连接 Redis: {}", config.url);
        let client = redis::Client::open(config.url.as_str()).context("无法创建 Redis 客户端")?;

        let conn = ConnectionManager::new(client)
            .await
            .context("无法连接到 Redis")?;

        info!("✅ Redis 连接成功");

        Ok(Self {
            conn,
            key_prefix: config.key_prefix,
        })
    }

    /// 生成 TextBox 的键
    fn text_box_key(&self, id: &str) -> String {
        format!("{}:textbox:{}", self.key_prefix, id)
    }

    /// 生成索引键（用于列表）
    fn index_key(&self) -> String {
        format!("{}:index", self.key_prefix)
    }

    /// 创建 TextBox
    pub async fn create(&mut self, text_box: TextBox) -> Result<TextBox> {
        let id = text_box.id.clone();
        let key = self.text_box_key(&id);

        // 序列化
        let data = serde_json::to_string(&text_box).context("序列化 TextBox 失败")?;

        // 存储到 Redis
        self.conn
            .set::<_, _, ()>(&key, data)
            .await
            .context("保存 TextBox 到 Redis 失败")?;

        // 添加到索引（使用 sorted set，按创建时间排序）
        let score = text_box.metadata.created_at.timestamp() as f64;
        self.conn
            .zadd::<_, _, _, ()>(&self.index_key(), &id, score)
            .await
            .context("添加到索引失败")?;

        info!("✅ 创建 TextBox: id={}, author={}", id, text_box.author);
        Ok(text_box)
    }

    /// 获取 TextBox
    pub async fn get(&mut self, id: &str) -> Result<Option<TextBox>> {
        let key = self.text_box_key(id);

        let data: Option<String> = self
            .conn
            .get(&key)
            .await
            .context("从 Redis 获取 TextBox 失败")?;

        match data {
            Some(json) => {
                let mut text_box: TextBox =
                    serde_json::from_str(&json).context("反序列化 TextBox 失败")?;

                // 增加浏览次数
                text_box.increment_view();

                // 更新到 Redis
                let updated_data = serde_json::to_string(&text_box)?;
                self.conn
                    .set::<_, _, ()>(&key, updated_data)
                    .await
                    .context("更新浏览次数失败")?;

                debug!(
                    "获取 TextBox: id={}, views={}",
                    id, text_box.metadata.view_count
                );
                Ok(Some(text_box))
            }
            None => {
                debug!("TextBox 不存在: id={}", id);
                Ok(None)
            }
        }
    }

    /// 列出 TextBox（分页）
    pub async fn list(&mut self, params: PaginationParams) -> Result<PaginatedResult<TextBox>> {
        let index_key = self.index_key();

        // 获取总数
        let total: u64 = self.conn.zcard(&index_key).await.context("获取总数失败")?;

        if total == 0 {
            return Ok(PaginatedResult::new(vec![], 0, &params));
        }

        // 计算范围（倒序，最新的在前）
        let offset = params.offset() as isize;
        let limit = params.limit() as isize;
        let end = -(offset + 1);
        let start = end - limit + 1;

        // 从 sorted set 获取 ID 列表（倒序）
        let ids: Vec<String> = self
            .conn
            .zrevrange(&index_key, start, end)
            .await
            .context("获取 ID 列表失败")?;

        // 批量获取 TextBox
        let mut items = Vec::new();
        for id in ids {
            if let Ok(Some(text_box)) = self.get_without_increment(&id).await {
                // 过滤掉过期的
                if !text_box.is_expired() {
                    items.push(text_box);
                }
            }
        }

        debug!(
            "列出 TextBox: page={}, page_size={}, total={}, items={}",
            params.page,
            params.page_size,
            total,
            items.len()
        );

        Ok(PaginatedResult::new(items, total, &params))
    }

    /// 获取 TextBox（不增加浏览次数）
    async fn get_without_increment(&mut self, id: &str) -> Result<Option<TextBox>> {
        let key = self.text_box_key(id);

        let data: Option<String> = self
            .conn
            .get(&key)
            .await
            .context("从 Redis 获取 TextBox 失败")?;

        match data {
            Some(json) => {
                let text_box: TextBox =
                    serde_json::from_str(&json).context("反序列化 TextBox 失败")?;
                Ok(Some(text_box))
            }
            None => Ok(None),
        }
    }

    /// 删除 TextBox
    pub async fn delete(&mut self, id: &str) -> Result<bool> {
        let key = self.text_box_key(id);

        // 从存储中删除
        let deleted: u32 = self.conn.del(&key).await.context("删除 TextBox 失败")?;

        // 从索引中删除
        self.conn
            .zrem::<_, _, ()>(&self.index_key(), id)
            .await
            .context("从索引删除失败")?;

        let success = deleted > 0;
        if success {
            info!("🗑️  删除 TextBox: id={}", id);
        }

        Ok(success)
    }

    /// 更新 TextBox
    pub async fn update(&mut self, text_box: TextBox) -> Result<TextBox> {
        let id = text_box.id.clone();
        let key = self.text_box_key(&id);

        // 检查是否存在
        let exists: bool = self
            .conn
            .exists(&key)
            .await
            .context("检查 TextBox 是否存在失败")?;

        if !exists {
            anyhow::bail!("TextBox 不存在: id={id}");
        }

        // 序列化并保存
        let data = serde_json::to_string(&text_box).context("序列化 TextBox 失败")?;

        self.conn
            .set::<_, _, ()>(&key, data)
            .await
            .context("更新 TextBox 到 Redis 失败")?;

        info!("✏️  更新 TextBox: id={}", id);
        Ok(text_box)
    }

    /// 清理过期的 TextBox
    pub async fn cleanup_expired(&mut self) -> Result<u32> {
        let index_key = self.index_key();

        // 获取所有 ID
        let ids: Vec<String> = self
            .conn
            .zrange(&index_key, 0, -1)
            .await
            .context("获取所有 ID 失败")?;

        let mut deleted_count = 0;

        for id in ids {
            if let Ok(Some(text_box)) = self.get_without_increment(&id).await
                && text_box.is_expired()
                && self.delete(&id).await?
            {
                deleted_count += 1;
            }
        }

        if deleted_count > 0 {
            info!("🧹 清理过期 TextBox: 删除 {} 个", deleted_count);
        }

        Ok(deleted_count)
    }

    /// 获取统计信息
    pub async fn stats(&mut self) -> Result<TextBoxStats> {
        let total: u64 = self
            .conn
            .zcard(self.index_key())
            .await
            .context("获取总数失败")?;

        Ok(TextBoxStats { total })
    }
}

impl Debug for TextBoxManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextBoxManager")
            .field("key_prefix", &self.key_prefix)
            .finish()
    }
}

/// 统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextBoxStats {
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TextBox;

    async fn create_test_manager() -> Result<TextBoxManager> {
        let config = RedisConfig::default().with_prefix("anybox_test".to_string());
        TextBoxManager::new(config).await
    }

    #[tokio::test]
    #[ignore] // 需要 Redis 运行
    async fn test_create_and_get() -> Result<()> {
        let mut manager = create_test_manager().await?;

        let text_box = TextBox::new("Alice".to_string(), "Hello, Redis!".to_string())
            .with_title("Test".to_string());

        let id = text_box.id.clone();
        let _created = manager.create(text_box).await?;

        let fetched = manager.get(&id).await?;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().author, "Alice");

        manager.delete(&id).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore] // 需要 Redis 运行
    async fn test_list_pagination() -> Result<()> {
        let mut manager = create_test_manager().await?;

        // 创建多个 TextBox
        for i in 0..5 {
            let text_box = TextBox::new(format!("User{}", i), format!("Content {}", i));
            manager.create(text_box).await?;
        }

        // 测试分页
        let params = PaginationParams::new(1, 2);
        let result = manager.list(params).await?;

        assert!(result.total >= 5);
        assert_eq!(result.page_size, 2);

        Ok(())
    }
}
