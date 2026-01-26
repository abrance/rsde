use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use mysql_async::{Pool, Row, params, prelude::*};
use tracing::{debug, info};

use crate::models::{PaginatedResult, PaginationParams, PromptCategory, PromptTemplate};

#[derive(Clone)]
pub struct PromptTemplateManager {
    pool: Pool,
    table_name: String,
}

impl PromptTemplateManager {
    pub async fn new(config: config::prompt::PromptConfig) -> Result<Self> {
        let url = config.mysql.connection_url();
        info!(
            "Connecting to MySQL: {}:{}",
            config.mysql.host, config.mysql.port
        );

        let pool = Pool::new(mysql_async::Opts::from_url(&url).context("Invalid MySQL URL")?);

        let manager = Self {
            pool,
            table_name: format!("{}templates", config.table_prefix),
        };

        manager.init_table().await?;
        info!("✅ MySQL connection successful");

        Ok(manager)
    }

    async fn init_table(&self) -> Result<()> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let create_table_sql = format!(
            r#"CREATE TABLE IF NOT EXISTS `{}` (
                `id` VARCHAR(36) NOT NULL PRIMARY KEY,
                `name` VARCHAR(255) NOT NULL,
                `description` TEXT,
                `category` VARCHAR(50) NOT NULL DEFAULT 'chat',
                `content` LONGTEXT NOT NULL,
                `variables` JSON,
                `tags` JSON,
                `version` INT UNSIGNED NOT NULL DEFAULT 1,
                `is_active` BOOLEAN NOT NULL DEFAULT TRUE,
                `created_at` DATETIME NOT NULL,
                `updated_at` DATETIME NOT NULL,
                `created_by` VARCHAR(255),
                INDEX `idx_name` (`name`),
                INDEX `idx_category` (`category`),
                INDEX `idx_is_active` (`is_active`),
                INDEX `idx_created_at` (`created_at`)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci"#,
            self.table_name
        );

        conn.query_drop(&create_table_sql)
            .await
            .context("Failed to create table")?;

        debug!("Table '{}' initialized", self.table_name);
        Ok(())
    }

    pub async fn create(&self, template: PromptTemplate) -> Result<PromptTemplate> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let variables_json = serde_json::to_string(&template.variables)?;
        let tags_json = serde_json::to_string(&template.tags)?;
        let created_at = template.created_at.naive_utc();
        let updated_at = template.updated_at.naive_utc();

        let insert_sql = format!(
            r#"INSERT INTO `{}` (id, name, description, category, content, variables, tags, version, is_active, created_at, updated_at, created_by)
               VALUES (:id, :name, :description, :category, :content, :variables, :tags, :version, :is_active, :created_at, :updated_at, :created_by)"#,
            self.table_name
        );

        conn.exec_drop(
            &insert_sql,
            params! {
                "id" => &template.id,
                "name" => &template.name,
                "description" => &template.description,
                "category" => template.category.as_str(),
                "content" => &template.content,
                "variables" => &variables_json,
                "tags" => &tags_json,
                "version" => template.version,
                "is_active" => template.is_active,
                "created_at" => created_at,
                "updated_at" => updated_at,
                "created_by" => &template.created_by,
            },
        )
        .await
        .context("Failed to insert prompt template")?;

        info!(
            "✅ Created PromptTemplate: id={}, name={}",
            template.id, template.name
        );
        Ok(template)
    }

    pub async fn get(&self, id: &str) -> Result<Option<PromptTemplate>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let select_sql = format!(
            "SELECT id, name, description, category, content, variables, tags, version, is_active, created_at, updated_at, created_by FROM `{}` WHERE id = :id",
            self.table_name
        );

        let result: Option<Row> = conn
            .exec_first(&select_sql, params! { "id" => id })
            .await
            .context("Failed to query prompt template")?;

        match result {
            Some(row) => {
                let template = self.row_to_template(row)?;
                debug!("Retrieved PromptTemplate: id={}", id);
                Ok(Some(template))
            }
            None => {
                debug!("PromptTemplate not found: id={}", id);
                Ok(None)
            }
        }
    }

    pub async fn list(&self, params: PaginationParams) -> Result<PaginatedResult<PromptTemplate>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let count_sql = format!("SELECT COUNT(*) FROM `{}`", self.table_name);
        let total: u64 = conn
            .query_first(&count_sql)
            .await
            .context("Failed to count templates")?
            .unwrap_or(0);

        if total == 0 {
            return Ok(PaginatedResult::new(vec![], 0, &params));
        }

        let select_sql = format!(
            "SELECT id, name, description, category, content, variables, tags, version, is_active, created_at, updated_at, created_by FROM `{}` ORDER BY created_at DESC LIMIT :limit OFFSET :offset",
            self.table_name
        );

        let rows: Vec<Row> = conn
            .exec(
                &select_sql,
                params! { "limit" => params.limit(), "offset" => params.offset() },
            )
            .await
            .context("Failed to list templates")?;

        let mut items = Vec::new();
        for row in rows {
            items.push(self.row_to_template(row)?);
        }

        debug!(
            "Listed PromptTemplates: page={}, page_size={}, total={}",
            params.page, params.page_size, total
        );
        Ok(PaginatedResult::new(items, total, &params))
    }

    pub async fn update(&self, template: PromptTemplate) -> Result<PromptTemplate> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let variables_json = serde_json::to_string(&template.variables)?;
        let tags_json = serde_json::to_string(&template.tags)?;
        let updated_at = template.updated_at.naive_utc();

        let update_sql = format!(
            r#"UPDATE `{}` SET name = :name, description = :description, category = :category, content = :content, 
               variables = :variables, tags = :tags, version = :version, is_active = :is_active, updated_at = :updated_at, created_by = :created_by
               WHERE id = :id"#,
            self.table_name
        );

        let affected = conn
            .exec_iter(
                &update_sql,
                params! {
                    "id" => &template.id,
                    "name" => &template.name,
                    "description" => &template.description,
                    "category" => template.category.as_str(),
                    "content" => &template.content,
                    "variables" => &variables_json,
                    "tags" => &tags_json,
                    "version" => template.version,
                    "is_active" => template.is_active,
                    "updated_at" => updated_at,
                    "created_by" => &template.created_by,
                },
            )
            .await
            .context("Failed to update prompt template")?
            .affected_rows();

        if affected == 0 {
            anyhow::bail!("PromptTemplate not found: id={}", template.id);
        }

        info!("✏️ Updated PromptTemplate: id={}", template.id);
        Ok(template)
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let delete_sql = format!("DELETE FROM `{}` WHERE id = :id", self.table_name);
        let affected = conn
            .exec_iter(&delete_sql, params! { "id" => id })
            .await
            .context("Failed to delete prompt template")?
            .affected_rows();

        let success = affected > 0;
        if success {
            info!("🗑️ Deleted PromptTemplate: id={}", id);
        }

        Ok(success)
    }

    pub async fn search_by_name(
        &self,
        name: &str,
        params: PaginationParams,
    ) -> Result<PaginatedResult<PromptTemplate>> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .context("Failed to get MySQL connection")?;

        let search_pattern = format!("%{name}%");

        let count_sql = format!(
            "SELECT COUNT(*) FROM `{}` WHERE name LIKE :pattern",
            self.table_name
        );
        let total: u64 = conn
            .exec_first(&count_sql, params! { "pattern" => &search_pattern })
            .await
            .context("Failed to count templates")?
            .unwrap_or(0);

        if total == 0 {
            return Ok(PaginatedResult::new(vec![], 0, &params));
        }

        let select_sql = format!(
            "SELECT id, name, description, category, content, variables, tags, version, is_active, created_at, updated_at, created_by FROM `{}` WHERE name LIKE :pattern ORDER BY created_at DESC LIMIT :limit OFFSET :offset",
            self.table_name
        );

        let rows: Vec<Row> = conn
            .exec(&select_sql, params! { "pattern" => &search_pattern, "limit" => params.limit(), "offset" => params.offset() })
            .await
            .context("Failed to search templates")?;

        let mut items = Vec::new();
        for row in rows {
            items.push(self.row_to_template(row)?);
        }

        Ok(PaginatedResult::new(items, total, &params))
    }

    fn row_to_template(&self, row: Row) -> Result<PromptTemplate> {
        let id: String = row.get("id").context("Missing id")?;
        let name: String = row.get("name").context("Missing name")?;
        let description: Option<String> = row.get("description");
        let category_str: String = row.get("category").context("Missing category")?;
        let content: String = row.get("content").context("Missing content")?;
        let variables_json: String = row.get("variables").unwrap_or_else(|| "[]".to_string());
        let tags_json: String = row.get("tags").unwrap_or_else(|| "[]".to_string());
        let version: u32 = row.get("version").context("Missing version")?;
        let is_active: bool = row.get("is_active").context("Missing is_active")?;
        let created_at: NaiveDateTime = row.get("created_at").context("Missing created_at")?;
        let updated_at: NaiveDateTime = row.get("updated_at").context("Missing updated_at")?;
        let created_by: Option<String> = row.get("created_by");

        let variables: Vec<String> = serde_json::from_str(&variables_json).unwrap_or_default();
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let category = PromptCategory::parse(&category_str).unwrap_or_default();

        Ok(PromptTemplate {
            id,
            name,
            description,
            category,
            content,
            variables,
            tags,
            version,
            is_active,
            created_at: chrono::DateTime::from_naive_utc_and_offset(created_at, chrono::Utc),
            updated_at: chrono::DateTime::from_naive_utc_and_offset(updated_at, chrono::Utc),
            created_by,
        })
    }
}

impl std::fmt::Debug for PromptTemplateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptTemplateManager")
            .field("table_name", &self.table_name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::mysql::MysqlConfig;

    fn create_test_config() -> config::prompt::PromptConfig {
        config::prompt::PromptConfig {
            mysql: MysqlConfig {
                host: std::env::var("MYSQL_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: std::env::var("MYSQL_PORT")
                    .unwrap_or_else(|_| "3306".to_string())
                    .parse()
                    .unwrap_or(3306),
                user: std::env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
                password: std::env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "".to_string()),
                database: "prompt_test".to_string(),
                ..Default::default()
            },
            table_prefix: "test_prompt_".to_string(),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_create_and_get() -> Result<()> {
        let config = create_test_config();
        let manager = PromptTemplateManager::new(config).await?;

        let template = PromptTemplate::new("test".to_string(), "Hello {{name}}!".to_string())
            .with_description("Test template".to_string())
            .with_variables(vec!["name".to_string()]);

        let id = template.id.clone();
        let created = manager.create(template).await?;

        let fetched = manager.get(&id).await?;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "test");

        manager.delete(&id).await?;
        Ok(())
    }
}
