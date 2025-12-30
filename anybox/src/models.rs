use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 文本格式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextFormat {
    /// 纯文本
    Plain,
    /// Markdown
    Markdown,
    /// 代码
    Code,
    /// JSON
    Json,
    /// XML
    Xml,
    /// HTML
    Html,
    /// YAML
    Yaml,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self::Plain
    }
}

impl TextFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Markdown => "markdown",
            Self::Code => "code",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Yaml => "yaml",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "plain" => Some(Self::Plain),
            "markdown" | "md" => Some(Self::Markdown),
            "code" => Some(Self::Code),
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            "html" => Some(Self::Html),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }
}

/// TextBox 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBoxMetadata {
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 过期时间（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// 浏览次数
    #[serde(default)]
    pub view_count: u64,
    /// 是否公开
    #[serde(default = "default_true")]
    pub is_public: bool,
    /// 语言/代码类型（可选，用于代码高亮）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for TextBoxMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            expires_at: None,
            view_count: 0,
            is_public: true,
            language: None,
            tags: Vec::new(),
        }
    }
}

/// 文本帖子
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBox {
    /// 唯一ID
    pub id: String,
    /// 作者姓名
    pub author: String,
    /// 标题（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 文本格式
    pub format: TextFormat,
    /// 文本内容
    pub content: String,
    /// 元数据
    pub metadata: TextBoxMetadata,
}

impl TextBox {
    /// 创建新的 TextBox
    pub fn new(author: String, content: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            author,
            title: None,
            format: TextFormat::default(),
            content,
            metadata: TextBoxMetadata::default(),
        }
    }

    /// 使用构建器模式设置各项属性
    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn with_format(mut self, format: TextFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_language(mut self, language: String) -> Self {
        self.metadata.language = Some(language);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags = tags;
        self
    }

    pub fn with_expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.metadata.expires_at = Some(expires_at);
        self
    }

    pub fn with_public(mut self, is_public: bool) -> Self {
        self.metadata.is_public = is_public;
        self
    }

    /// 增加浏览次数
    pub fn increment_view(&mut self) {
        self.metadata.view_count += 1;
        self.metadata.updated_at = Utc::now();
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.metadata.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// 更新内容
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.metadata.updated_at = Utc::now();
    }
}

/// 分页参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    /// 页码（从1开始）
    #[serde(default = "default_page")]
    pub page: u32,
    /// 每页数量
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
        }
    }
}

impl PaginationParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.min(100).max(1),
        }
    }

    /// 计算偏移量
    pub fn offset(&self) -> usize {
        ((self.page - 1) * self.page_size) as usize
    }

    /// 计算限制数量
    pub fn limit(&self) -> usize {
        self.page_size as usize
    }
}

/// 分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    /// 数据列表
    pub items: Vec<T>,
    /// 总数量
    pub total: u64,
    /// 当前页码
    pub page: u32,
    /// 每页数量
    pub page_size: u32,
    /// 总页数
    pub total_pages: u32,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, params: &PaginationParams) -> Self {
        let total_pages = ((total as f64) / (params.page_size as f64)).ceil() as u32;
        Self {
            items,
            total,
            page: params.page,
            page_size: params.page_size,
            total_pages: total_pages.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_box_creation() {
        let text_box = TextBox::new("Alice".to_string(), "Hello, world!".to_string());
        assert_eq!(text_box.author, "Alice");
        assert_eq!(text_box.content, "Hello, world!");
        assert_eq!(text_box.format, TextFormat::Plain);
        assert!(!text_box.is_expired());
    }

    #[test]
    fn test_text_box_builder() {
        let text_box = TextBox::new("Bob".to_string(), "```rust\nfn main() {}```".to_string())
            .with_title("My Code".to_string())
            .with_format(TextFormat::Code)
            .with_language("rust".to_string())
            .with_tags(vec!["rust".to_string(), "example".to_string()]);

        assert_eq!(text_box.title, Some("My Code".to_string()));
        assert_eq!(text_box.format, TextFormat::Code);
        assert_eq!(text_box.metadata.language, Some("rust".to_string()));
        assert_eq!(text_box.metadata.tags.len(), 2);
    }

    #[test]
    fn test_pagination_params() {
        let params = PaginationParams::new(2, 50);
        assert_eq!(params.page, 2);
        assert_eq!(params.page_size, 50);
        assert_eq!(params.offset(), 50);
        assert_eq!(params.limit(), 50);

        // Test limits
        let params = PaginationParams::new(0, 200);
        assert_eq!(params.page, 1); // Min page
        assert_eq!(params.page_size, 100); // Max page_size
    }
}
