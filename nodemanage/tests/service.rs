use chrono::{TimeDelta, TimeZone, Utc};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkService, DataLinkStatus,
    DataSourceInput, DataType, EtlMode, EtlPipelineInput, ResultTableInput, StorageType,
    storage::memory::MemoryDataLinkRepository,
};
use nodemanage::{
    AgentRegistration, AgentRunMode, AgentSyncRequest, BindingState, CreateNode, HeartbeatConfig,
    InstallNodeRequest, InstallStatus, JobManageConfig, MemoryNodeRepository, NodeManageError,
    NodeManager, NodeStatus, NoopRsAgentInstaller, OnlineStatus, PaginationParams,
    SyncBindingState, TaskFilterDefaults, UpdateNode,
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
        .create(CreateNode {
            name: "worker-1".to_string(),
            endpoint: "http://worker-1:8080".to_string(),
            labels: vec![],
        })
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
