use crate::controller::Controller;
use crate::rule::{DataTransferConfig, GlobalConfigData};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

pub struct RuleFileWatcher {
    controller: Arc<Mutex<Controller>>,
    watch_dir: String,
    loaded_files: Vec<String>,
    global_config: Option<GlobalConfigData>,
}

impl RuleFileWatcher {
    pub fn new(controller: Arc<Mutex<Controller>>, watch_dir: String) -> Self {
        Self {
            controller,
            watch_dir,
            loaded_files: Vec::new(),
            global_config: None,
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

        // 首先检查全局配置文件
        let global_config_path = path.join("config.toml");
        if global_config_path.exists() {
            let global_config_file = global_config_path.file_name().unwrap().to_string_lossy().to_string();
            if !self.loaded_files.contains(&global_config_file) {
                match self.load_global_config(&global_config_path).await {
                    Ok(_) => {
                        println!("Successfully loaded global config from {:?}", global_config_path);
                        self.loaded_files.push(global_config_file);
                    }
                    Err(e) => {
                        eprintln!("Failed to load global config from {:?}: {}", global_config_path, e);
                        self.loaded_files.push(global_config_file);
                    }
                }
            }
        }

        // 然后处理管道配置文件
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "toml" {
                    let file_name = path.file_name().unwrap().to_string_lossy().to_string();

                    // Skip Cargo.toml and config.toml (already handled)
                    if file_name == "Cargo.toml" || file_name == "config.toml" {
                        continue;
                    }

                    // 处理以 .rule.toml 结尾的文件作为管道配置
                    if file_name.ends_with(".rule.toml") && !self.loaded_files.contains(&file_name) {
                        println!("Found new pipeline config file: {file_name}");
                        if let Err(e) = self.load_pipeline_config(&path).await {
                            eprintln!("Failed to load pipeline config from {file_name}: {e}");
                            // Add to loaded_files to prevent infinite retry loop on bad config
                            self.loaded_files.push(file_name);
                        } else {
                            self.loaded_files.push(file_name);
                        }
                    }
                }
            }
        }
    }

    async fn load_global_config(&mut self, path: &Path) -> anyhow::Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {e}"))?;

        let config: GlobalConfigData =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("Parse error: {e}"))?;

        self.global_config = Some(config);
        Ok(())
    }

    async fn load_pipeline_config(&self, path: &Path) -> anyhow::Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {e}"))?;

        // 使用 toml crate 解析配置
        let mut config: DataTransferConfig =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("Parse error: {e}"))?;

        // 如果有全局配置，合并到管道配置中
        if let Some(global_config) = &self.global_config {
            config = config.with_global_config(global_config);
        }

        let mut controller = self.controller.lock().await;
        controller
            .add_config(config)
            .await
            .map_err(|e| anyhow::anyhow!("Controller error: {e}"))?;

        println!("Successfully loaded and started pipeline from {path:?}");
        Ok(())
    }
}
