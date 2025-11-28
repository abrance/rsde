use crate::controller::Controller;
use crate::rule::DataTransferConfig;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

pub struct RuleFileWatcher {
    controller: Arc<Mutex<Controller>>,
    watch_dir: String,
    loaded_files: Vec<String>,
}

impl RuleFileWatcher {
    pub fn new(controller: Arc<Mutex<Controller>>, watch_dir: String) -> Self {
        Self {
            controller,
            watch_dir,
            loaded_files: Vec::new(),
        }
    }

    pub async fn run(&mut self) {
        let mut interval = time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            self.scan_and_load().await;
        }
    }

    async fn scan_and_load(&mut self) {
        let path = Path::new(&self.watch_dir);
        if !path.exists() || !path.is_dir() {
            eprintln!("Watch directory does not exist: {}", self.watch_dir);
            return;
        }

        let mut entries = match tokio::fs::read_dir(path).await {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Failed to read directory {}: {}", self.watch_dir, e);
                return;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "toml" {
                    let file_name = path.file_name().unwrap().to_string_lossy().to_string();

                    // Skip Cargo.toml
                    if file_name == "Cargo.toml" {
                        continue;
                    }

                    if !self.loaded_files.contains(&file_name) {
                        println!("Found new config file: {}", file_name);
                        if let Err(e) = self.load_config(&path).await {
                            eprintln!("Failed to load config from {}: {}", file_name, e);
                            // Add to loaded_files to prevent infinite retry loop on bad config
                            // In a real system, we might want a separate "failed" list or retry with backoff
                            self.loaded_files.push(file_name);
                        } else {
                            self.loaded_files.push(file_name);
                        }
                    }
                }
            }
        }
    }

    async fn load_config(&self, path: &Path) -> anyhow::Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {}", e))?;

        // 使用 toml crate 解析配置
        // 注意：这里假设 DataTransferConfig 实现了 Deserialize
        // 并且其中的 Box<dyn Source> 等字段可以通过 typetag 正确反序列化
        let config: DataTransferConfig =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

        let mut controller = self.controller.lock().await;
        controller
            .add_config(config)
            .await
            .map_err(|e| anyhow::anyhow!("Controller error: {}", e))?;

        println!("Successfully loaded and started pipeline from {:?}", path);
        Ok(())
    }
}
