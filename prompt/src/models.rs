use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PromptCategory {
    Chat,
    Completion,
    Assistant,
    Agent,
    Custom,
}

impl Default for PromptCategory {
    fn default() -> Self {
        Self::Chat
    }
}

impl PromptCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Completion => "completion",
            Self::Assistant => "assistant",
            Self::Agent => "agent",
            Self::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "chat" => Some(Self::Chat),
            "completion" => Some(Self::Completion),
            "assistant" => Some(Self::Assistant),
            "agent" => Some(Self::Agent),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: PromptCategory,
    pub content: String,
    pub variables: Vec<String>,
    pub tags: Vec<String>,
    pub version: u32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

impl PromptTemplate {
    pub fn new(name: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description: None,
            category: PromptCategory::default(),
            content,
            variables: Vec::new(),
            tags: Vec::new(),
            version: 1,
            is_active: true,
            created_at: now,
            updated_at: now,
            created_by: None,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_category(mut self, category: PromptCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_variables(mut self, variables: Vec<String>) -> Self {
        self.variables = variables;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_created_by(mut self, created_by: String) -> Self {
        self.created_by = Some(created_by);
        self
    }

    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.version += 1;
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
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
            page_size: page_size.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> u64 {
        ((self.page - 1) * self.page_size) as u64
    }

    pub fn limit(&self) -> u64 {
        self.page_size as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
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
    fn test_prompt_template_creation() {
        let template = PromptTemplate::new("test".to_string(), "Hello {{name}}!".to_string());
        assert_eq!(template.name, "test");
        assert_eq!(template.version, 1);
        assert!(template.is_active);
    }

    #[test]
    fn test_prompt_template_builder() {
        let template = PromptTemplate::new(
            "assistant".to_string(),
            "You are a helpful assistant.".to_string(),
        )
        .with_description("Default assistant prompt".to_string())
        .with_category(PromptCategory::Assistant)
        .with_variables(vec!["context".to_string()])
        .with_tags(vec!["default".to_string(), "assistant".to_string()]);

        assert_eq!(
            template.description,
            Some("Default assistant prompt".to_string())
        );
        assert_eq!(template.category, PromptCategory::Assistant);
        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.tags.len(), 2);
    }

    #[test]
    fn test_pagination_params() {
        let params = PaginationParams::new(2, 50);
        assert_eq!(params.page, 2);
        assert_eq!(params.page_size, 50);
        assert_eq!(params.offset(), 50);
        assert_eq!(params.limit(), 50);

        let params = PaginationParams::new(0, 200);
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 100);
    }
}
