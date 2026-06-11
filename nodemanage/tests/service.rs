use chrono::{TimeDelta, TimeZone, Utc};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkService, DataLinkStatus,
    DataSourceInput, DataType, EtlMode, EtlPipelineInput, ResultTableInput, StorageType,
    storage::memory::MemoryDataLinkRepository,
};
use nodemanage::{
    AgentRegistration, AgentRunMode, AgentSyncRequest, BindingState, CreateNode, HeartbeatConfig,
    InstallNodeRequest, InstallStatus, InstallTaskState, InstallTaskStep, JobManageConfig,
    MemoryNodeRepository, NodeAgentBinding, NodeInstallTask, NodeManageError, NodeManager,
    NodeRepository, NodeStatus, NoopRsAgentInstaller, OnlineStatus, PaginationParams,
    RebindNodeRequest, ResolvedInstallRequest, SyncBindingState, TaskFilterDefaults, UpdateNode,
};
use query_engine::{HeartbeatSample, InMemoryHeartbeatStore, QueryEngine};
use std::{collections::HashMap, time::Duration};

fn manager() -> NodeManager<MemoryNodeRepository, NoopRsAgentInstaller> {
    NodeManager::new(MemoryNodeRepository::default(), NoopRsAgentInstaller)
}

fn manager_with_repository(
    repository: MemoryNodeRepository,
) -> NodeManager<MemoryNodeRepository, NoopRsAgentInstaller> {
    NodeManager::new(repository, NoopRsAgentInstaller)
}

fn heartbeat_spec(result_table_name: &str) -> ApplyDataLinkSpec {
    ApplyDataLinkSpec {
        name: "node-heartbeat".to_string(),
        description: Some("shared node heartbeat".to_string()),
        domain: "nodemanage".to_string(),
        owner_service: "nodemanage".to_string(),
        data_type: DataType::Metric,
        status: DataLinkStatus::Active,
        status_message: None,
        datasource: DataSourceInput {
            producer: "rsagent".to_string(),
            data_type: DataType::Metric,
            collect_method: CollectMethod::Push,
            protocol: Some("http".to_string()),
            interval_seconds: Some(60),
            labels: HashMap::new(),
            dimension_keys: vec!["node_id".to_string()],
            auth_ref: None,
            config: HashMap::new(),
        },
        etl_pipeline: EtlPipelineInput {
            mode: EtlMode::Passthrough,
            config: HashMap::new(),
        },
        result_table: ResultTableInput {
            result_table_name: result_table_name.to_string(),
            storage_type: StorageType::Victoriametrics,
            storage_cluster: Some("vm-cluster-a".to_string()),
            database: None,
            table_name: None,
            metric_name: Some("nm_node_heartbeat".to_string()),
            query_template: Some("query heartbeat".to_string()),
            schema: HashMap::new(),
            retention_days: Some(7),
        },
    }
}

fn sync_request(
    agent_id: &str,
    node_id: Option<&str>,
    config_version: Option<&str>,
) -> AgentSyncRequest {
    AgentSyncRequest {
        agent_id: agent_id.to_string(),
        node_id: node_id.map(ToString::to_string),
        agent_version: "0.1.0".to_string(),
        hostname: format!("{agent_id}.example.internal"),
        os_family: "linux".to_string(),
        os_distribution: "ubuntu".to_string(),
        arch: "x86_64".to_string(),
        capabilities: vec!["heartbeat".to_string(), "task-sync".to_string()],
        started_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        config_version: config_version.map(ToString::to_string),
    }
}

#[tokio::test]
async fn manager_can_create_and_get_node() {
    let manager = manager();

    let created = manager
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec!["gpu".to_string()],
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
        })
        .await
        .unwrap();
    let fetched = manager.get(&created.id).await.unwrap();

    assert_eq!(fetched, Some(created));
}

#[tokio::test]
async fn manager_rejects_create_node_with_blank_required_fields() {
    let manager = manager();

    let error = manager
        .create(CreateNode::simple("   ", ""))
        .await
        .expect_err("blank create-node input should fail");

    match error {
        NodeManageError::InvalidInput(message) => {
            assert!(message.contains("name"));
            assert!(message.contains("endpoint"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn manager_rejects_create_node_with_duplicate_endpoint() {
    let manager = manager();

    manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let error = manager
        .create(CreateNode::simple("worker-2", "http://worker-1:8080"))
        .await
        .expect_err("duplicate endpoint should fail");

    match error {
        NodeManageError::Conflict(message) => {
            assert!(message.contains("endpoint"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn manager_updates_node_fields() {
    let manager = manager();
    let created = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
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
                ..Default::default()
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
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
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
        .install_node(ResolvedInstallRequest {
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
async fn manager_returns_install_task_by_id() {
    let repository = MemoryNodeRepository::default();
    let task = repository
        .create_install_task(NodeInstallTask::new("node-1".to_string()))
        .await
        .unwrap();
    let manager = manager_with_repository(repository);

    let loaded = manager
        .install_task(&task.install_task_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(loaded.install_task_id, task.install_task_id);
    assert_eq!(loaded.node_id, "node-1");
    assert_eq!(loaded.task_state, "pending");
}

#[tokio::test]
async fn manager_returns_latest_install_task_for_node() {
    let repository = MemoryNodeRepository::default();
    let mut older = NodeInstallTask::new("node-1".to_string());
    older.task_state = InstallTaskState::Failed;
    older.current_step = Some(InstallTaskStep::RunInstallScript);
    repository.create_install_task(older).await.unwrap();

    let mut newer = NodeInstallTask::new("node-1".to_string());
    newer.task_state = InstallTaskState::Running;
    newer.current_step = Some(InstallTaskStep::StartAgent);
    let newer = repository.create_install_task(newer).await.unwrap();

    let manager = manager_with_repository(repository);

    let latest = manager
        .latest_install_task("node-1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(latest.install_task_id, newer.install_task_id);
    assert_eq!(latest.current_step.as_deref(), Some("start_agent"));
}

#[tokio::test]
async fn list_node_summaries_derives_environment_and_state_axes() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());
    let node = manager
        .create(CreateNode {
            name: "worker-proj".to_string(),
            endpoint: "http://worker-proj:8080".to_string(),
            labels: vec!["environment:test".to_string(), "edge".to_string()],
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
        })
        .await
        .unwrap();

    let mut task = NodeInstallTask::new(node.id.clone());
    task.task_state = InstallTaskState::Running;
    task.current_step = Some(InstallTaskStep::StartAgent);
    repository.create_install_task(task).await.unwrap();

    let summaries = manager
        .list_node_summaries(PaginationParams::new(1, 10))
        .await
        .unwrap();

    assert_eq!(summaries.total, 1);
    assert_eq!(summaries.items[0].node_id, node.id);
    assert_eq!(summaries.items[0].environment, "test");
    assert_eq!(summaries.items[0].install_phase, "running");
    assert_eq!(summaries.items[0].binding_state, "unbound");
    assert_eq!(summaries.items[0].online_status, "offline");
    assert_eq!(summaries.items[0].lifecycle_state, "onboarding");
}

#[tokio::test]
async fn get_node_detail_aggregates_binding_status_latest_task_and_heartbeat_ref() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());
    let node = manager
        .create(CreateNode {
            name: "worker-detail".to_string(),
            endpoint: "http://worker-detail:8080".to_string(),
            labels: vec!["env:prod".to_string()],
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
        })
        .await
        .unwrap();

    let binding = repository
        .upsert_agent_binding(nodemanage::NodeAgentBinding::new(
            node.id.clone(),
            "agent-detail".to_string(),
        ))
        .await
        .unwrap();
    let mut task = NodeInstallTask::new(node.id.clone());
    task.task_state = InstallTaskState::Succeeded;
    let task = repository.create_install_task(task).await.unwrap();

    let detail = manager
        .node_detail(
            &node.id,
            Some("dl-heartbeat".to_string()),
            Some("nm_node_heartbeat".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(detail.node.node_id, node.id);
    assert_eq!(detail.node.environment, "prod");
    assert_eq!(detail.binding.as_ref().unwrap().agent_id, binding.agent_id);
    assert_eq!(detail.status.install_phase, "succeeded");
    assert_eq!(detail.status.binding_state, "bound");
    assert_eq!(
        detail.latest_install_task.as_ref().unwrap().install_task_id,
        task.install_task_id
    );
    assert_eq!(
        detail.heartbeat_ref.as_ref().unwrap().data_link_id,
        "dl-heartbeat"
    );
}

#[tokio::test]
async fn status_batch_returns_projection_for_requested_nodes_only() {
    let manager = manager();
    let first = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();
    let second = manager
        .create(CreateNode::simple("worker-2", "http://worker-2:8080"))
        .await
        .unwrap();

    let batch = manager.batch_status(vec![first.id.clone()]).await.unwrap();

    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].node_id, first.id);
    assert_ne!(batch[0].node_id, second.id);
}

#[tokio::test]
async fn submit_install_task_persists_request_context() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple(
            "worker-install-context",
            "http://worker-install-context:8080",
        ))
        .await
        .unwrap();

    let request = InstallNodeRequest {
        host: Some("10.0.0.8".to_string()),
        ssh_port: Some(22),
        username: Some("root".to_string()),
        password: Some("secret".to_string()),
        private_key: None,
        rsagent_package_url: Some("https://example.com/rsagent.tar.gz".to_string()),
        install_root: Some("/opt/rsagent".to_string()),
        register_callback_url: Some("http://127.0.0.1:3000/api/nm/v1/agents/sync".to_string()),
        plugins: vec![],
        labels: vec!["edge".to_string()],
    };

    let receipt = manager
        .submit_install_task(&node.id, request)
        .await
        .unwrap();
    let task = manager
        .install_task(&receipt.install_task_id)
        .await
        .unwrap()
        .unwrap();
    let task_json = serde_json::to_value(&task).unwrap();

    assert_eq!(task_json["request_summary"]["host"], "10.0.0.8");
    assert_eq!(task_json["request_summary"]["ssh_port"], 22);
    assert_eq!(task_json["request_summary"]["username"], "root");
    assert_eq!(
        task_json["request_summary"]["labels"],
        serde_json::json!(["edge"])
    );
    assert!(task_json.get("request_host").is_none());
    assert!(task_json.get("password").is_none());
    assert!(task_json.get("private_key").is_none());
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

#[tokio::test]
async fn sync_first_sync_creates_binding_and_returns_active_config() {
    let manager = manager();

    let response = manager
        .sync_agent(sync_request("agent-1", Some("node-1"), None))
        .await
        .unwrap();
    let binding = manager.agent_binding("agent-1").await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.agent_id, "agent-1");
    assert_eq!(response.bound_node_id, "node-1");
    assert_eq!(response.binding_state, SyncBindingState::Bound);
    assert_eq!(response.agent_run_mode, AgentRunMode::Active);
    assert_eq!(response.config_version, "2026-05-29T10:00:00Z");
    assert_eq!(binding.agent_id, "agent-1");
    assert_eq!(binding.node_id, "node-1");
    assert_eq!(binding.binding_state, BindingState::Bound);
}

#[tokio::test]
async fn sync_later_sync_refreshes_handshake_and_config() {
    let manager = manager();

    manager
        .sync_agent(sync_request("agent-1", Some("node-1"), None))
        .await
        .unwrap();
    let first_binding = manager.agent_binding("agent-1").await.unwrap();

    tokio::time::sleep(Duration::from_millis(5)).await;

    let response = manager
        .sync_agent(sync_request(
            "agent-1",
            Some("node-1"),
            Some("stale-config-version"),
        ))
        .await
        .unwrap();
    let refreshed_binding = manager.agent_binding("agent-1").await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.bound_node_id, "node-1");
    assert_eq!(response.binding_state, SyncBindingState::Bound);
    assert_eq!(response.config_version, "2026-05-29T10:00:00Z");
    assert!(refreshed_binding.last_handshake_at > first_binding.last_handshake_at);
}

#[tokio::test]
async fn sync_later_sync_without_node_id_reuses_existing_binding() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());

    manager
        .sync_agent(sync_request("agent-1", Some("node-1"), None))
        .await
        .unwrap();
    let first_binding = manager.agent_binding("agent-1").await.unwrap();

    tokio::time::sleep(Duration::from_millis(5)).await;

    let reloaded_manager = manager_with_repository(repository);
    let response = reloaded_manager
        .sync_agent(sync_request("agent-1", None, Some("stale-config-version")))
        .await
        .unwrap();
    let refreshed_binding = reloaded_manager.agent_binding("agent-1").await.unwrap();

    assert!(response.accepted);
    assert_eq!(response.bound_node_id, "node-1");
    assert_eq!(response.binding_state, SyncBindingState::Bound);
    assert_eq!(response.agent_run_mode, AgentRunMode::Active);
    assert_eq!(response.rejection_reason, None);
    assert!(refreshed_binding.last_handshake_at > first_binding.last_handshake_at);
}

#[tokio::test]
async fn sync_initial_sync_without_node_id_returns_explicit_unbound_rejection() {
    let manager = manager();

    let response = manager
        .sync_agent(sync_request("agent-1", None, None))
        .await
        .unwrap();

    assert!(!response.accepted);
    assert_eq!(response.agent_id, "agent-1");
    assert_eq!(response.bound_node_id, "");
    assert_eq!(response.binding_state, SyncBindingState::Unbound);
    assert_eq!(response.agent_run_mode, AgentRunMode::Idle);
    assert_eq!(
        response.rejection_reason,
        Some("node_id is required for initial sync".to_string())
    );
    assert!(manager.agent_binding("agent-1").await.is_none());
}

#[tokio::test]
async fn sync_conflict_binding_is_explicit_and_not_silently_overwritten() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());

    manager
        .sync_agent(sync_request("agent-1", Some("node-1"), None))
        .await
        .unwrap();

    let reloaded_manager = manager_with_repository(repository);
    let response = reloaded_manager
        .sync_agent(sync_request("agent-2", Some("node-1"), None))
        .await
        .unwrap();

    assert!(!response.accepted);
    assert_eq!(response.agent_id, "agent-2");
    assert_eq!(response.bound_node_id, "node-1");
    assert_eq!(response.binding_state, SyncBindingState::Conflict);
    assert_eq!(response.agent_run_mode, AgentRunMode::Idle);
    assert_eq!(
        response.rejection_reason,
        Some("node node-1 is already bound to agent agent-1".to_string())
    );

    let preserved_binding = reloaded_manager.agent_binding("agent-1").await.unwrap();
    assert_eq!(preserved_binding.node_id, "node-1");
    assert!(reloaded_manager.agent_binding("agent-2").await.is_none());
}

#[tokio::test]
async fn rebind_node_promotes_target_binding_and_marks_previous_binding_stale() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());
    let node = manager
        .create(CreateNode::simple(
            "worker-rebind",
            "http://worker-rebind:8080",
        ))
        .await
        .unwrap();

    manager
        .sync_agent(sync_request("agent-old", Some(&node.id), None))
        .await
        .unwrap();

    let mut target_binding =
        NodeAgentBinding::new("detached-node".to_string(), "agent-new".to_string());
    target_binding.binding_state = BindingState::Stale;
    target_binding.unbind_reason = Some("waiting_rebind".to_string());
    repository
        .upsert_agent_binding(target_binding)
        .await
        .unwrap();

    let response = manager
        .rebind_node(
            &node.id,
            &RebindNodeRequest {
                target_agent_id: "agent-new".to_string(),
                reason: Some("manual_rebind_after_conflict".to_string()),
            },
        )
        .await
        .unwrap();

    assert!(response.accepted);
    assert_eq!(response.node_id, node.id);
    assert_eq!(response.target_agent_id, "agent-new");
    assert_eq!(response.binding_state, "bound");
    assert_eq!(response.previous_agent_id.as_deref(), Some("agent-old"));

    let stale_binding = manager.agent_binding("agent-old").await.unwrap();
    assert_eq!(stale_binding.binding_state, BindingState::Stale);
    assert_eq!(stale_binding.node_id, node.id);

    let active_binding = manager.agent_binding("agent-new").await.unwrap();
    assert_eq!(active_binding.binding_state, BindingState::Bound);
    assert_eq!(active_binding.node_id, node.id);
    assert_eq!(active_binding.unbind_reason, None);
}

#[tokio::test]
async fn rebind_node_rejects_blank_target_agent_id() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple(
            "worker-rebind-invalid",
            "http://worker-rebind-invalid:8080",
        ))
        .await
        .unwrap();

    let err = manager
        .rebind_node(
            &node.id,
            &RebindNodeRequest {
                target_agent_id: "   ".to_string(),
                reason: None,
            },
        )
        .await
        .unwrap_err();

    match err {
        NodeManageError::InvalidRebindRequest(message) => {
            assert!(message.contains("target_agent_id"));
        }
        other => panic!("expected invalid rebind request, got {other:?}"),
    }
}

#[tokio::test]
async fn rebind_node_returns_target_agent_not_found_when_binding_missing() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple(
            "worker-rebind-missing-agent",
            "http://worker-rebind-missing-agent:8080",
        ))
        .await
        .unwrap();

    let err = manager
        .rebind_node(
            &node.id,
            &RebindNodeRequest {
                target_agent_id: "agent-missing".to_string(),
                reason: None,
            },
        )
        .await
        .unwrap_err();

    match err {
        NodeManageError::TargetAgentNotFound(agent_id) => {
            assert_eq!(agent_id, "agent-missing");
        }
        other => panic!("expected target agent not found, got {other:?}"),
    }
}

#[tokio::test]
async fn rebind_node_returns_conflict_when_target_agent_is_bound_to_other_node() {
    let repository = MemoryNodeRepository::default();
    let manager = manager_with_repository(repository.clone());
    let node = manager
        .create(CreateNode::simple(
            "worker-rebind-conflict",
            "http://worker-rebind-conflict:8080",
        ))
        .await
        .unwrap();

    manager
        .sync_agent(sync_request("agent-old", Some(&node.id), None))
        .await
        .unwrap();

    repository
        .upsert_agent_binding(NodeAgentBinding::new(
            "other-node".to_string(),
            "agent-target".to_string(),
        ))
        .await
        .unwrap();

    let err = manager
        .rebind_node(
            &node.id,
            &RebindNodeRequest {
                target_agent_id: "agent-target".to_string(),
                reason: Some("manual_rebind_after_conflict".to_string()),
            },
        )
        .await
        .unwrap_err();

    match err {
        NodeManageError::RebindTargetAlreadyBound(message) => {
            assert!(message.contains("agent-target"));
            assert!(message.contains("other-node"));
        }
        other => panic!("expected rebind target already bound, got {other:?}"),
    }
}

#[tokio::test]
async fn rebind_node_returns_not_found_for_missing_node() {
    let repository = MemoryNodeRepository::default();
    repository
        .upsert_agent_binding(NodeAgentBinding::new(
            "some-node".to_string(),
            "agent-target".to_string(),
        ))
        .await
        .unwrap();
    let manager = manager_with_repository(repository);

    let err = manager
        .rebind_node(
            "missing-node",
            &RebindNodeRequest {
                target_agent_id: "agent-target".to_string(),
                reason: None,
            },
        )
        .await
        .unwrap_err();

    match err {
        NodeManageError::NotFound(node_id) => {
            assert_eq!(node_id, "missing-node");
        }
        other => panic!("expected node not found, got {other:?}"),
    }
}

#[tokio::test]
async fn sync_response_includes_runtime_and_polling_config_fields() {
    let manager = manager();

    let response = manager
        .sync_agent(sync_request("agent-1", Some("node-1"), None))
        .await
        .unwrap();

    assert_eq!(
        response.heartbeat_config,
        HeartbeatConfig {
            version: "1".to_string(),
            data_link_id: "dl_heartbeat_001".to_string(),
            vm_base_url: "http://victoriametrics:8428".to_string(),
            interval_secs: 60,
        }
    );
    assert_eq!(
        response.job_manage_config,
        JobManageConfig {
            version: "1".to_string(),
            base_url: "http://job-manage:3000/api/job-manage/v1/tasks".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec![
                    "queued".to_string(),
                    "dispatched".to_string(),
                    "acknowledged".to_string(),
                    "running".to_string(),
                ],
            },
        }
    );
    assert_eq!(response.sync_interval_secs, 30);
    assert_eq!(response.task_sync_interval_secs, 10);
}

#[tokio::test]
async fn refresh_status_marks_node_online_when_heartbeat_is_fresh() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let bundle = datalink_service
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();
    let heartbeat_store = InMemoryHeartbeatStore::new();
    heartbeat_store.insert(
        "nm_node_heartbeat",
        HeartbeatSample {
            node_id: node.id.clone(),
            observed_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        },
    );
    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let refreshed = manager
        .refresh_status_from_query(
            &node.id,
            &engine,
            &bundle.data_link.data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap();

    assert_eq!(refreshed.status, NodeStatus::Online);
    assert_eq!(
        refreshed.last_heartbeat_at,
        Some(Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap())
    );
}

#[tokio::test]
async fn refresh_status_marks_node_offline_when_heartbeat_is_missing() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let bundle = datalink_service
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();
    let engine = QueryEngine::new(datalink_service, InMemoryHeartbeatStore::new());

    let refreshed = manager
        .refresh_status_from_query(
            &node.id,
            &engine,
            &bundle.data_link.data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap();

    assert_eq!(refreshed.status, NodeStatus::Offline);
    assert_eq!(refreshed.last_heartbeat_at, None);
}

#[tokio::test]
async fn refresh_status_uses_stable_data_link_id_after_result_table_rename() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let bundle = datalink_service
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat_v1"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();
    let updated_bundle = datalink_service
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat_v2"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat-updated".to_string()),
            },
        )
        .unwrap();

    assert_eq!(
        bundle.data_link.data_link_id, updated_bundle.data_link.data_link_id,
        "logical re-apply should preserve stable data_link_id"
    );

    let heartbeat_store = InMemoryHeartbeatStore::new();
    heartbeat_store.insert(
        "nm_node_heartbeat_v2",
        HeartbeatSample {
            node_id: node.id.clone(),
            observed_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        },
    );
    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let refreshed = manager
        .refresh_status_from_query(
            &node.id,
            &engine,
            &bundle.data_link.data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap();

    assert_eq!(refreshed.status, NodeStatus::Online);
    assert_eq!(
        refreshed.last_heartbeat_at,
        Some(Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap())
    );
}

#[tokio::test]
async fn refresh_status_returns_storage_error_when_heartbeat_data_link_id_no_longer_resolves() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let issued_bundle = DataLinkService::new(MemoryDataLinkRepository::new())
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();

    let stale_data_link_id = issued_bundle.data_link.data_link_id.clone();
    let engine = QueryEngine::new(
        DataLinkService::new(MemoryDataLinkRepository::new()),
        InMemoryHeartbeatStore::new(),
    );

    let err = manager
        .refresh_status_from_query(
            &node.id,
            &engine,
            &stale_data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap_err();

    match err {
        NodeManageError::Storage(message) => {
            assert!(message.contains(&stale_data_link_id));
        }
        other => panic!("expected storage error, got {other:?}"),
    }
}

#[tokio::test]
async fn aggregate_status_snapshot_from_query_reports_online_for_fresh_heartbeat() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let bundle = datalink_service
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();
    let heartbeat_store = InMemoryHeartbeatStore::new();
    heartbeat_store.insert(
        "nm_node_heartbeat",
        HeartbeatSample {
            node_id: node.id.clone(),
            observed_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        },
    );
    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let snapshot = manager
        .aggregate_status_snapshot_from_query(
            &node.id,
            &engine,
            &bundle.data_link.data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap();

    assert_eq!(snapshot.node_id, node.id);
    assert_eq!(snapshot.online_status, OnlineStatus::Online);
    assert!(snapshot.status_reason.is_none());
}

#[tokio::test]
async fn aggregate_status_snapshot_from_query_reports_unknown_when_query_path_breaks() {
    let manager = manager();
    let node = manager
        .create(CreateNode::simple("worker-1", "http://worker-1:8080"))
        .await
        .unwrap();

    let issued_bundle = DataLinkService::new(MemoryDataLinkRepository::new())
        .apply_data_link(
            heartbeat_spec("nm_node_heartbeat"),
            ApplyDataLinkOptions {
                idempotency_key: Some("nm-heartbeat".to_string()),
            },
        )
        .unwrap();
    let stale_data_link_id = issued_bundle.data_link.data_link_id.clone();
    let engine = QueryEngine::new(
        DataLinkService::new(MemoryDataLinkRepository::new()),
        InMemoryHeartbeatStore::new(),
    );

    let snapshot = manager
        .aggregate_status_snapshot_from_query(
            &node.id,
            &engine,
            &stale_data_link_id,
            Utc.with_ymd_and_hms(2026, 5, 29, 10, 3, 0).unwrap(),
            TimeDelta::minutes(5),
        )
        .await
        .unwrap();

    assert_eq!(snapshot.node_id, node.id);
    assert_eq!(snapshot.online_status, OnlineStatus::Unknown);
    assert!(
        snapshot
            .status_reason
            .as_deref()
            .is_some_and(|reason| reason.contains(&stale_data_link_id))
    );
}
