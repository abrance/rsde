use nodemanage::{
    AgentRegistration, BindingState, CreateNode, InstallNodeRequest, InstallPlugin, InstallStatus,
    MemoryNodeRepository, Node, NodeAgentBinding, NodeRepository, NodeStatus, NodeStatusSnapshot,
    OnlineStatus, PaginationParams,
};

#[test]
fn node_new_assigns_id_and_timestamps() {
    let node = Node::new(
        "worker-1".to_string(),
        "http://worker-1:8080".to_string(),
        vec!["gpu".to_string()],
    );

    assert!(!node.id.is_empty());
    assert_eq!(node.name, "worker-1");
    assert_eq!(node.endpoint, "http://worker-1:8080");
    assert_eq!(node.status, NodeStatus::Offline);
    assert_eq!(node.labels, vec!["gpu".to_string()]);
    assert_eq!(node.created_at, node.updated_at);
    assert!(node.last_heartbeat_at.is_none());
}

#[test]
fn node_status_parse_accepts_supported_values() {
    assert_eq!(NodeStatus::parse("online"), Some(NodeStatus::Online));
    assert_eq!(NodeStatus::parse("offline"), Some(NodeStatus::Offline));
    assert_eq!(
        NodeStatus::parse("maintenance"),
        Some(NodeStatus::Maintenance)
    );
    assert_eq!(NodeStatus::parse("unknown"), None);
}

#[tokio::test]
async fn memory_repository_can_create_fetch_and_list_nodes() {
    let repository = MemoryNodeRepository::default();
    let node = Node::new(
        "worker-1".to_string(),
        "http://worker-1:8080".to_string(),
        vec!["gpu".to_string()],
    );

    let created = repository.create(node.clone()).await.unwrap();
    let fetched = repository.get(&created.id).await.unwrap();
    let listed = repository.list(PaginationParams::new(1, 10)).await.unwrap();

    assert_eq!(created.name, node.name);
    assert_eq!(fetched, Some(created));
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items.len(), 1);
}

#[test]
fn create_node_builds_node_with_offline_status() {
    let input = CreateNode {
        name: "worker-2".to_string(),
        endpoint: "http://worker-2:8080".to_string(),
        labels: vec!["cpu".to_string()],
    };

    let node = input.into_node();

    assert_eq!(node.name, "worker-2");
    assert_eq!(node.status, NodeStatus::Offline);
}

#[test]
fn install_request_records_ssh_target_and_rsagent_source() {
    let request = InstallNodeRequest {
        host: "10.0.0.8".to_string(),
        ssh_port: 2222,
        username: "root".to_string(),
        password: Some("secret".to_string()),
        private_key: None,
        rsagent_package_url: "https://example.com/rsagent.tar.gz".to_string(),
        install_root: "/opt/rsagent".to_string(),
        register_callback_url: "http://127.0.0.1:3000/api/nodes/agent/register".to_string(),
        plugins: vec![InstallPlugin {
            name: "metrics".to_string(),
            version: "1.2.3".to_string(),
            package_url: Some("https://example.com/plugins/metrics.tar.gz".to_string()),
        }],
        labels: vec!["edge".to_string()],
    };

    assert_eq!(request.host, "10.0.0.8");
    assert_eq!(request.ssh_port, 2222);
    assert_eq!(
        request.rsagent_package_url,
        "https://example.com/rsagent.tar.gz"
    );
    assert_eq!(request.install_root, "/opt/rsagent");
    assert_eq!(
        request.register_callback_url,
        "http://127.0.0.1:3000/api/nodes/agent/register"
    );
    assert_eq!(request.plugins.len(), 1);
    assert_eq!(request.plugins[0].name, "metrics");
    assert_eq!(request.plugins[0].version, "1.2.3");
    assert_eq!(request.labels, vec!["edge".to_string()]);
}

#[test]
fn install_request_defaults_install_root() {
    let request: InstallNodeRequest = serde_json::from_str(
        r#"{
            "host": "10.0.0.9",
            "username": "root",
            "password": "secret",
            "rsagent_package_url": "https://example.com/rsagent.tar.gz",
            "register_callback_url": "http://127.0.0.1:3000/api/nodes/agent/register"
        }"#,
    )
    .unwrap();

    assert_eq!(request.install_root, "/opt/rsagent");
    assert!(request.plugins.is_empty());
}

#[test]
fn agent_registration_turns_into_online_node() {
    let registration = AgentRegistration {
        agent_id: "agent-1".to_string(),
        hostname: "worker-3".to_string(),
        endpoint: "http://worker-3:19090".to_string(),
        labels: vec!["ssh-installed".to_string()],
    };

    let node = registration.into_node();

    assert_eq!(node.name, "worker-3");
    assert_eq!(node.endpoint, "http://worker-3:19090");
    assert_eq!(node.status, NodeStatus::Online);
    assert_eq!(node.labels, vec!["ssh-installed".to_string()]);
    assert!(node.last_heartbeat_at.is_some());
}

#[test]
fn install_status_serializes_supported_states() {
    assert_eq!(InstallStatus::Pending.as_str(), "pending");
    assert_eq!(InstallStatus::Installing.as_str(), "installing");
    assert_eq!(InstallStatus::WaitingRegister.as_str(), "waiting_register");
    assert_eq!(InstallStatus::Registered.as_str(), "registered");
    assert_eq!(InstallStatus::Failed.as_str(), "failed");
}

#[test]
fn node_identity_is_independent_of_agent_bindings() {
    let node = Node::new(
        "worker-1".to_string(),
        "http://worker-1:8080".to_string(),
        vec!["gpu".to_string()],
    );
    let node_id = node.id.clone();

    let binding = NodeAgentBinding::new(node_id.clone(), "agent-abc".to_string());

    assert_eq!(binding.node_id, node.id);
    assert_ne!(binding.agent_id, node.id);
    assert_eq!(binding.binding_state, BindingState::Bound);
}

#[test]
fn node_agent_binding_records_registration_timestamps() {
    let binding = NodeAgentBinding::new("node-1".to_string(), "agent-1".to_string());

    assert_eq!(binding.node_id, "node-1");
    assert_eq!(binding.agent_id, "agent-1");
    assert_eq!(binding.binding_state, BindingState::Bound);
    assert!(binding.first_registered_at <= binding.last_handshake_at);
    assert!(binding.unbind_reason.is_none());
}

#[test]
fn one_node_can_have_multiple_agent_bindings() {
    let node_id = "node-1".to_string();

    let binding_a = NodeAgentBinding::new(node_id.clone(), "agent-1".to_string());
    let binding_b = NodeAgentBinding::new(node_id.clone(), "agent-2".to_string());

    assert_eq!(binding_a.node_id, binding_b.node_id);
    assert_ne!(binding_a.agent_id, binding_b.agent_id);

    let bindings = vec![binding_a, binding_b];
    assert_eq!(bindings.len(), 2);

    let bound_count = bindings
        .iter()
        .filter(|b| b.binding_state == BindingState::Bound)
        .count();
    assert_eq!(bound_count, 2);
}

#[test]
fn binding_state_transitions_are_explicit() {
    let mut binding = NodeAgentBinding::new("node-1".to_string(), "agent-1".to_string());
    assert_eq!(binding.binding_state, BindingState::Bound);

    binding.binding_state = BindingState::Stale;
    assert_eq!(binding.binding_state, BindingState::Stale);

    binding.binding_state = BindingState::Unbound;
    binding.unbind_reason = Some("agent-restarted".to_string());
    assert_eq!(binding.binding_state, BindingState::Unbound);
    assert!(binding.unbind_reason.is_some());
}

#[test]
fn node_status_snapshot_is_a_read_model() {
    let snapshot = NodeStatusSnapshot::new("node-1".to_string(), OnlineStatus::Online, None);

    assert_eq!(snapshot.node_id, "node-1");
    assert_eq!(snapshot.online_status, OnlineStatus::Online);
    assert!(snapshot.status_reason.is_none());
    assert!(snapshot.aggregated_at <= chrono::Utc::now());
}

#[test]
fn node_status_snapshot_can_be_offline_with_reason() {
    let snapshot = NodeStatusSnapshot::new(
        "node-1".to_string(),
        OnlineStatus::Offline,
        Some("no heartbeat for 5 minutes".to_string()),
    );

    assert_eq!(snapshot.online_status, OnlineStatus::Offline);
    assert_eq!(
        snapshot.status_reason,
        Some("no heartbeat for 5 minutes".to_string())
    );
}
