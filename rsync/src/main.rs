use rule::{
    DataTransferConfig,
    controller::Controller,
    file::{FileSinkConfig, FileSourceConfig, RsyncEnv},
};

#[tokio::main]
async fn main() {
    let file_source = FileSourceConfig {
        path: "/opt/mystorage/github/rsde/rsync/lib/rule/README.md".to_string(),
        watch: false,
    };
    let env = RsyncEnv::detect();
    let target_path = "/tmp/README_copy.md".to_string();
    let force = true;
    const DEFAULT_FILE_MASK: &str = "rw-r--r--";
    let mask = Some(DEFAULT_FILE_MASK.to_string());

    let file_sink = FileSinkConfig::new(env, target_path, force, mask);

    println!("Created file sink for: {}", file_sink.path);
    println!("Created file source for: {}", file_source.path);
    let data_transfer_config = DataTransferConfig::new(
        "example_id".to_string(),
        "Example Data Transfer".to_string(),
        Some("An example data transfer configuration".to_string()),
        vec![Box::new(file_source)],
        vec![],
        vec![Box::new(file_sink)],
    );
    println!("Data Transfer Config ID: {:?}", data_transfer_config);

    let mut controller = Controller::new();
    match controller.add_config(data_transfer_config).await {
        Ok(_) => println!("Controller started successfully"),
        Err(e) => eprintln!("Error starting controller: {}", e),
    }

    // Keep the main thread alive to let the controller run
    println!("Running... Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await.unwrap();
    println!("Shutdown signal received");
}
