#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file() {
        // 创建一个临时的 TOML 配置文件用于测试
        let test_config = r#"
[metadata]
id = "test-pipeline"
name = "Test Pipeline"
description = "A test pipeline for logging configuration"

[[sources]]
source_type = "file"
path = "/tmp/test_input.txt"
watch = true

[[transforms]]
transform_type = "json"
add_timestamp = true

[[sinks]]
sink_type = "file"
path = "/tmp/test_output.txt"
force = true
env = { platform = { kernel = "Linux", arch = "X86_64", distribution = "Unknown" } }

[api]
listen_address = "0.0.0.0:8080"
log_level = "debug"
metrics_enabled = true

[global]
debug = true

[log]
path = "./test_logs/"
"#;

        // 将测试配置写入临时文件
        std::fs::write("test_config.toml", test_config).unwrap();

        // 测试 from_file 方法
        let config = DataTransferConfig::from_file("test_config.toml");
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.metadata.id, "test-pipeline");
        assert_eq!(config.api.log_level, "debug");
        assert_eq!(config.log.path, "./test_logs/");

        // 清理临时文件
        std::fs::remove_file("test_config.toml").unwrap();
    }
}