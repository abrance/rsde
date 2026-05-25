use nodemanage::{
    AgentRegistration, CreateNode, InstallNodeRequest, InstallStatus, MemoryNodeRepository,
    NodeManager, NodeStatus, NoopRsAgentInstaller, PaginationParams, UpdateNode,
};

fn manager() -> NodeManager<MemoryNodeRepository, NoopRsAgentInstaller> {
    NodeManager::new(MemoryNodeRepository::default(), NoopRsAgentInstaller)
}

#[tokio::test]
async fn manager_can_create_and_get_node() {
    let manager = manager();

    let created = manager
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec!["gpu".to_string()],
        })
        .await
        .unwrap();
    let fetched = manager.get(&created.id).await.unwrap();

    assert_eq!(fetched, Some(created));
}

#[tokio::test]
async fn manager_updates_node_fields() {
    let manager = manager();
    let created = manager
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
        .await
        .unwrap();

    let updated = manager
        .update(
            &created.id,
            UpdateNode {
                name: Some("worker-renamed".to_string()),
                endpoint: None,
                status: Some(NodeStatus::Maintenance),
                labels: Some(vec!["maintenance".to_string()]),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "worker-renamed");
    assert_eq!(updated.status, NodeStatus::Maintenance);
    assert_eq!(updated.labels, vec!["maintenance".to_string()]);
    assert!(updated.updated_at >= created.updated_at);
}

#[tokio::test]
async fn manager_delete_returns_false_for_missing_node() {
    let manager = manager();

    let deleted = manager.delete("missing").await.unwrap();

    assert!(!deleted);
}

#[tokio::test]
async fn heartbeat_marks_node_online_and_sets_timestamp() {
    let manager = manager();
    let created = manager
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
        .await
        .unwrap();

    let heartbeat = manager.heartbeat(&created.id).await.unwrap();

    assert_eq!(heartbeat.status, NodeStatus::Online);
    assert!(heartbeat.last_heartbeat_at.is_some());
}

#[tokio::test]
async fn install_node_delegates_to_rsagent_installer() {
    let manager = manager();

    let result = manager
        .install_node(InstallNodeRequest {
            host: "10.0.0.8".to_string(),
            ssh_port: 22,
            username: "root".to_string(),
            password: Some("secret".to_string()),
            private_key: None,
            rsagent_package_url: "https://example.com/rsagent.tar.gz".to_string(),
            install_root: "/opt/rsagent".to_string(),
            register_callback_url: "http://127.0.0.1:3000/api/nodes/agent/register".to_string(),
            plugins: vec![],
            labels: vec![],
        })
        .await
        .unwrap();

    assert_eq!(result.host, "10.0.0.8");
    assert_eq!(result.status, InstallStatus::Pending);
}

#[tokio::test]
async fn register_agent_creates_online_node_record() {
    let manager = manager();

    let registered = manager
        .register_agent(AgentRegistration {
            agent_id: "agent-1".to_string(),
            hostname: "worker-registered".to_string(),
            endpoint: "http://worker-registered:19090".to_string(),
            labels: vec!["rsagent".to_string()],
        })
        .await
        .unwrap();
    let nodes = manager.list(PaginationParams::new(1, 10)).await.unwrap();

    assert_eq!(registered.id, "agent-1");
    assert_eq!(registered.status, NodeStatus::Online);
    assert_eq!(nodes.total, 1);
}

#[tokio::test]
async fn register_agent_is_idempotent_for_same_agent_id() {
    let manager = manager();

    let first = manager
        .register_agent(AgentRegistration {
            agent_id: "agent-1".to_string(),
            hostname: "worker-registered".to_string(),
            endpoint: "http://worker-registered:19090".to_string(),
            labels: vec!["rsagent".to_string()],
        })
        .await
        .unwrap();

    let second = manager
        .register_agent(AgentRegistration {
            agent_id: "agent-1".to_string(),
            hostname: "worker-registered".to_string(),
            endpoint: "http://worker-registered:29090".to_string(),
            labels: vec!["rsagent".to_string(), "gpu".to_string()],
        })
        .await
        .unwrap();

    let nodes = manager.list(PaginationParams::new(1, 10)).await.unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(second.endpoint, "http://worker-registered:29090");
    assert_eq!(
        second.labels,
        vec!["rsagent".to_string(), "gpu".to_string()]
    );
    assert_eq!(nodes.total, 1);
    assert_eq!(nodes.items[0].id, "agent-1");
}
