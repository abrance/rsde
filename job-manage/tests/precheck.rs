use job_manage::PrecheckService;
use nodemanage::{CreateNode, MemoryNodeRepository, NodeManager, NodeStatus, NoopRsAgentInstaller};

fn service() -> PrecheckService<MemoryNodeRepository, NoopRsAgentInstaller> {
    PrecheckService::new(NodeManager::new(
        MemoryNodeRepository::default(),
        NoopRsAgentInstaller,
    ))
}

#[tokio::test]
async fn precheck_passes_when_node_is_online() {
    let repository = MemoryNodeRepository::default();
    let manager = NodeManager::new(repository.clone(), NoopRsAgentInstaller);
    let service = PrecheckService::new(manager.clone());

    let node = manager
        .create(CreateNode {
            name: "worker-online".to_string(),
            endpoint: "http://worker-online:8080".to_string(),
            labels: vec![],
        })
        .await
        .unwrap();

    manager.heartbeat(&node.id).await.unwrap();

    let result = service.precheck(&node.id).await.unwrap();

    assert!(result.allowed);
    assert_eq!(result.status, NodeStatus::Online);
}

#[tokio::test]
async fn precheck_fails_when_node_is_offline() {
    let repository = MemoryNodeRepository::default();
    let manager = NodeManager::new(repository.clone(), NoopRsAgentInstaller);
    let service = PrecheckService::new(manager.clone());

    let node = manager
        .create(CreateNode {
            name: "worker-offline".to_string(),
            endpoint: "http://worker-offline:8080".to_string(),
            labels: vec![],
        })
        .await
        .unwrap();

    let result = service.precheck(&node.id).await.unwrap();

    assert!(!result.allowed);
    assert_eq!(result.status, NodeStatus::Offline);
}

#[tokio::test]
async fn precheck_fails_when_node_does_not_exist() {
    let service = service();

    let err = service.precheck("missing-node").await.unwrap_err();

    assert!(err.to_string().contains("missing-node"));
}
