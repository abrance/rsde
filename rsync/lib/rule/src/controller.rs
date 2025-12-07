use crate::event::Event;
use crate::rule::*;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct Controller {
    tasks: HashMap<String, JoinHandle<()>>,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Controller {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub async fn add_config(&mut self, config: DataTransferConfig) -> Result<()> {
        // 检查 metadata 是否存在
        let metadata = match &config.metadata {
            Some(metadata) => metadata,
            None => {
                return Err(RsyncError::ConfigError(
                    "Missing metadata in config".to_string(),
                ))
            }
        };

        let pipeline_id = metadata.id.clone();

        // 创建通道，用于连接 Source 和 Sink
        let (tx, mut rx) = mpsc::channel::<Box<dyn Event>>(100);

        // 1. 构建并启动 Sources
        for (index, source_config) in config.sources.iter().enumerate() {
            let source_id = format!("{pipeline_id}-source-{index}");
            let cx = SourceContext {
                key: ComponentKey::from(source_id.clone()),
                acknowledgements: source_config.can_acknowledge(),
            };

            let mut source_runtime = source_config.build(cx).await?;
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                loop {
                    match source_runtime.next_event().await {
                        Ok(Some(event)) => {
                            if tx_clone.send(event).await.is_err() {
                                break; // Channel closed
                            }
                        }
                        Ok(None) => break, // Source exhausted
                        Err(e) => {
                            eprintln!("Source error in {source_id}: {e}");
                            // 简单的错误处理：暂停一下
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            });
        }

        // Drop original tx so rx closes when all sources are done
        drop(tx);

        // 2. 构建 Transforms
        let mut transform_runtimes = Vec::new();
        for (index, transform_config) in config.transforms.iter().enumerate() {
            let transform_id = format!("{pipeline_id}-transform-{index}");
            let cx = TransformContext {
                key: ComponentKey::from(transform_id),
            };
            let runtime = transform_config.build(cx).await?;
            transform_runtimes.push(runtime);
        }

        // 3. 构建 Sinks
        let mut sink_runtimes = Vec::new();
        for (index, sink_config) in config.sinks.iter().enumerate() {
            let sink_id = format!("{pipeline_id}-sink-{index}");
            let cx = SinkContext {
                key: ComponentKey::from(sink_id),
                acknowledgements: false, // 简化
            };
            let runtime = sink_config.build(cx).await?;
            sink_runtimes.push(runtime);
        }

        // 4. 启动主循环处理 (Transform & Sink)
        let handle = tokio::spawn(async move {
            while let Some(initial_event) = rx.recv().await {
                let mut events = vec![initial_event];

                // Apply transforms
                for transform in &mut transform_runtimes {
                    let mut next_events = Vec::new();
                    for e in events {
                        match transform.process(e).await {
                            Ok(processed) => next_events.extend(processed),
                            Err(err) => eprintln!("Transform error: {err}"),
                        }
                    }
                    events = next_events;
                }

                // 分发给所有 Sinks
                for event in events {
                    if sink_runtimes.is_empty() {
                        continue;
                    }

                    // 处理前 N-1 个 sink
                    for i in 0..sink_runtimes.len() - 1 {
                        let event_clone = event.clone();
                        if let Err(e) = sink_runtimes[i].write(event_clone).await {
                            eprintln!("Sink write error: {e}");
                        }
                    }

                    // 处理最后一个 sink
                    if let Some(last_sink) = sink_runtimes.last_mut() {
                        if let Err(e) = last_sink.write(event).await {
                            eprintln!("Sink write error: {e}");
                        }
                    }
                }
            }

            // 管道结束，清理资源
            for sink in &mut sink_runtimes {
                let _ = sink.shutdown().await;
            }
        });

        self.tasks.insert(pipeline_id, handle);
        Ok(())
    }
}
