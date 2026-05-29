use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkService, DataLinkStatus,
    DataSourceInput, DataType, EtlMode, EtlPipelineInput, ResultTableInput, StorageType,
    storage::memory::MemoryDataLinkRepository,
};
use query_engine::{HeartbeatSample, InMemoryHeartbeatStore, QueryEngine};

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
            dimension_keys: vec!["node_id".to_string(), "agent_id".to_string()],
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

#[test]
fn latest_heartbeat_returns_record_for_known_node() {
    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    datalink_service
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
            node_id: "node-1".to_string(),
            observed_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        },
    );

    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let sample = engine
        .latest_heartbeat("nm_node_heartbeat", "node-1")
        .unwrap()
        .expect("heartbeat sample should exist");

    assert_eq!(sample.node_id, "node-1");
    assert_eq!(
        sample.observed_at,
        Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap()
    );
}

#[test]
fn latest_heartbeat_fails_when_configured_result_table_has_no_datalink() {
    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let heartbeat_store = InMemoryHeartbeatStore::new();
    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let err = engine
        .latest_heartbeat("missing_heartbeat_table", "node-1")
        .unwrap_err();

    assert!(err.to_string().contains("missing_heartbeat_table"));
}

#[test]
fn latest_heartbeat_by_data_link_id_returns_record_for_known_node() {
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
        &bundle.result_table.result_table_name,
        HeartbeatSample {
            node_id: "node-1".to_string(),
            observed_at: Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap(),
        },
    );

    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let sample = engine
        .latest_heartbeat_by_data_link_id(&bundle.data_link.data_link_id, "node-1")
        .unwrap()
        .expect("heartbeat sample should exist");

    assert_eq!(sample.node_id, "node-1");
    assert_eq!(
        sample.observed_at,
        Utc.with_ymd_and_hms(2026, 5, 29, 10, 0, 0).unwrap()
    );
}

#[test]
fn latest_heartbeat_by_data_link_id_fails_when_datalink_is_missing() {
    let datalink_service = DataLinkService::new(MemoryDataLinkRepository::new());
    let heartbeat_store = InMemoryHeartbeatStore::new();
    let engine = QueryEngine::new(datalink_service, heartbeat_store);

    let err = engine
        .latest_heartbeat_by_data_link_id("missing-datalink", "node-1")
        .unwrap_err();

    assert!(err.to_string().contains("missing-datalink"));
}
