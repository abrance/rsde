# Anybox - 文本分享服务

Anybox 是一个类似 [paste.ubuntu.com](https://paste.ubuntu.com) 的文本分享服务，使用 Redis 作为后端存储。

## 功能特性

- ✅ 创建文本帖子（TextBox）
- ✅ 多种文本格式支持（Plain, Markdown, Code, JSON, XML, HTML, YAML）
- ✅ 元数据管理（创建时间、浏览次数、标签、语言等）
- ✅ 分页列表
- ✅ 自动过期清理
- ✅ 公开/私有设置
- ✅ 代码语法高亮支持（language 字段）

## 架构设计

```
┌─────────────┐
│   用户请求   │
└──────┬──────┘
       │
       v
┌─────────────────────────────┐
│    API Server (Axum)        │
│  /api/anybox/textbox        │
└──────────┬──────────────────┘
           │
           v
┌─────────────────────────────┐
│  TextBoxManager             │
│  - create()                 │
│  - get()                    │
│  - list()                   │
│  - delete()                 │
└──────────┬──────────────────┘
           │
           v
┌─────────────────────────────┐
│  Redis Storage              │
│  - anybox:textbox:{id}      │
│  - anybox:index (sorted set)│
└─────────────────────────────┘
```

## 数据结构

### TextBox

```rust
pub struct TextBox {
    pub id: String,                    // UUID
    pub author: String,                // 作者姓名
    pub title: Option<String>,         // 标题（可选）
    pub format: TextFormat,            // 文本格式
    pub content: String,               // 文本内容
    pub metadata: TextBoxMetadata,     // 元数据
}
```

### TextBoxMetadata

```rust
pub struct TextBoxMetadata {
    pub created_at: DateTime<Utc>,     // 创建时间
    pub updated_at: DateTime<Utc>,     // 更新时间
    pub expires_at: Option<DateTime>,  // 过期时间
    pub view_count: u64,               // 浏览次数
    pub is_public: bool,               // 是否公开
    pub language: Option<String>,      // 语言/代码类型
    pub tags: Vec<String>,             // 标签
}
```

### TextFormat

支持的格式：
- `plain` - 纯文本
- `markdown` / `md` - Markdown
- `code` - 代码
- `json` - JSON
- `xml` - XML
- `html` - HTML
- `yaml` / `yml` - YAML

## API 接口

### 1. 创建 TextBox

**端点**: `POST /api/anybox/textbox`

**请求体**:
```json
{
  "author": "Alice",
  "content": "Hello, world!",
  "title": "My First Post",
  "format": "plain",
  "language": "rust",
  "tags": ["example", "test"],
  "is_public": true,
  "expire_hours": 24
}
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "author": "Alice",
    "title": "My First Post",
    "format": "plain",
    "content": "Hello, world!",
    "metadata": {
      "created_at": "2025-12-26T10:00:00Z",
      "updated_at": "2025-12-26T10:00:00Z",
      "expires_at": "2025-12-27T10:00:00Z",
      "view_count": 0,
      "is_public": true,
      "language": "rust",
      "tags": ["example", "test"]
    }
  }
}
```

### 2. 获取 TextBox

**端点**: `GET /api/anybox/textbox/:id`

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "author": "Alice",
    "content": "Hello, world!",
    "metadata": {
      "view_count": 5
    }
  }
}
```

**注意**: 每次获取会自动增加 `view_count`

### 3. 列出 TextBox（分页）

**端点**: `GET /api/anybox/textbox?page=1&page_size=20`

**查询参数**:
- `page`: 页码（从1开始，默认1）
- `page_size`: 每页数量（默认20，最大100）

**响应**:
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "...",
        "author": "Alice",
        "content": "..."
      }
    ],
    "total": 100,
    "page": 1,
    "page_size": 20,
    "total_pages": 5
  }
}
```

### 4. 删除 TextBox

**端点**: `DELETE /api/anybox/textbox/:id`

**响应**:
```json
{
  "success": true
}
```

### 5. 健康检查

**端点**: `GET /api/anybox/health`

**响应**:
```json
{
  "status": "ok",
  "service": "anybox-api",
  "version": "0.1.0"
}
```

## 配置

在 `manifest/dev/remote_ocr.toml` 中添加：

```toml
[anybox]
# Redis 连接 URL
redis_url = "redis://127.0.0.1:6379"
# 键前缀
key_prefix = "anybox"
# 清理过期内容的间隔时间（秒），默认 3600（1小时）
cleanup_interval_secs = 3600
```

## 使用示例

### 启动服务

```bash
# 确保 Redis 运行
redis-server

# 启动 API Server
cd /opt/mystorage/github/rsde
API_CONFIG=manifest/dev/remote_ocr.toml cargo run -p apiserver

# 查看日志
# INFO anybox::storage: 连接 Redis: redis://127.0.0.1:6379
# INFO anybox::storage: ✅ Redis 连接成功
# INFO apiserver::anybox: 启动 Anybox 清理任务: 间隔=3600秒
```

### 创建帖子

```bash
curl -X POST http://localhost:3000/api/anybox/textbox \
  -H "Content-Type: application/json" \
  -d '{
    "author": "Alice",
    "content": "# Hello, Anybox!\n\nThis is my first post.",
    "title": "First Post",
    "format": "markdown",
    "tags": ["introduction"]
  }' | jq
```

### 获取帖子

```bash
# 使用返回的 ID
curl http://localhost:3000/api/anybox/textbox/{id} | jq
```

### 列出帖子

```bash
# 第1页，每页10条
curl "http://localhost:3000/api/anybox/textbox?page=1&page_size=10" | jq

# 第2页
curl "http://localhost:3000/api/anybox/textbox?page=2&page_size=10" | jq
```

### 删除帖子

```bash
curl -X DELETE http://localhost:3000/api/anybox/textbox/{id} | jq
```

## Redis 数据结构

### 存储键模式

```
anybox:textbox:{id}      # 单个 TextBox 数据（JSON）
anybox:index             # Sorted Set，按创建时间排序的 ID 索引
```

### 示例

```bash
# 查看所有 TextBox ID
redis-cli ZRANGE anybox:index 0 -1

# 查看最新的 5 个
redis-cli ZREVRANGE anybox:index 0 4

# 查看总数
redis-cli ZCARD anybox:index

# 查看某个 TextBox
redis-cli GET anybox:textbox:{id}
```

## 自动清理

- 服务器启动时自动开启清理任务
- 每隔 `cleanup_interval_secs` 秒执行一次
- 删除 `expires_at` 已过期的 TextBox
- 清理日志：

```
INFO apiserver::anybox: 清理过期 TextBox: 删除 3 个
```

## 程序化使用

### Rust 库

```rust
use anybox::{RedisConfig, TextBoxManager, TextBox, TextFormat};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建管理器
    let config = RedisConfig::new("redis://127.0.0.1:6379".to_string())
        .with_prefix("anybox".to_string());
    let mut manager = TextBoxManager::new(config).await?;

    // 创建帖子
    let text_box = TextBox::new("Bob".to_string(), "```rust\nfn main() {}```".to_string())
        .with_title("Rust Example".to_string())
        .with_format(TextFormat::Code)
        .with_language("rust".to_string());

    let created = manager.create(text_box).await?;
    println!("Created: {}", created.id);

    // 获取帖子
    if let Some(fetched) = manager.get(&created.id).await? {
        println!("Views: {}", fetched.metadata.view_count);
    }

    // 列出帖子
    let params = PaginationParams::new(1, 10);
    let result = manager.list(params).await?;
    println!("Total: {}", result.total);

    // 删除帖子
    manager.delete(&created.id).await?;

    Ok(())
}
```

## 故障排查

### Redis 连接失败

```
ERROR: 无法连接到 Redis
```

**解决**:
1. 检查 Redis 是否运行：`redis-cli ping`
2. 检查连接 URL 配置
3. 检查防火墙设置

### 配置未加载

服务器启动时没有 "✅ 启用 Anybox 服务" 日志。

**解决**:
- 确保配置文件中有 `[anybox]` 部分
- 检查配置文件路径是否正确

### 过期清理不工作

**检查**:
- 查看日志是否有清理任务启动信息
- 检查 `cleanup_interval_secs` 配置
- 手动调用清理：

```rust
manager.cleanup_expired().await?;
```

## 测试

```bash
# 运行单元测试（需要 Redis）
cargo test -p anybox -- --ignored

# 运行所有测试
cargo test -p anybox
```

## 性能考虑

- **索引**: 使用 Redis Sorted Set 作为索引，O(log N) 查询
- **分页**: 使用 ZREVRANGE 倒序分页，高效
- **连接池**: 使用 ConnectionManager 管理连接
- **并发**: Mutex 保护共享状态，支持并发访问

## 扩展建议

### 生产环境

1. **添加认证**: 防止未授权访问
2. **添加限流**: 防止滥用
3. **Redis 集群**: 高可用和扩展性
4. **缓存优化**: 热门帖子缓存
5. **搜索功能**: 使用 RediSearch 或 Elasticsearch

### 功能增强

1. **富文本编辑器**: 前端集成 Monaco Editor 或 CodeMirror
2. **语法高亮**: 集成 highlight.js
3. **分享链接**: 短链接生成
4. **评论系统**: 添加评论功能
5. **统计分析**: 访问统计、热门帖子排行

## 相关资源

- [Redis Documentation](https://redis.io/docs/)
- [paste.ubuntu.com](https://paste.ubuntu.com)
- [Pastebin](https://pastebin.com)
