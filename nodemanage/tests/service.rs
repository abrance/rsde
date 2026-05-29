use chrono::{TimeDelta, TimeZone, Utc};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkService, DataLinkStatus,
    DataSourceInput, DataType, EtlMode, EtlPipelineInput, ResultTableInput, StorageType,
    storage::memory::MemoryDataLinkRepository,
};
use nodemanage::{
    AgentRegistration, CreateNode, InstallNodeRequest, InstallStatus, MemoryNodeRepository,
    NodeManageError, NodeManager, NodeStatus, NoopRsAgentInstaller, PaginationParams, UpdateNode,
};
use query_engine::{HeartbeatSample, InMemoryHeartbeatStore, QueryEngine};
use std::collections::HashMap;

fn manager() -> NodeManager<MemoryNodeRepository, NoopRsAgentInstaller> {
    NodeManager::new(MemoryNodeRepository::default(), NoopRsAgentInstaller)
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
