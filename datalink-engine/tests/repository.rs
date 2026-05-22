use std::collections::HashMap;

use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkError, DataLinkListFilter,
    DataLinkRepository, DataLinkStatus, DataSourceInput, DataType, EtlMode, EtlPipelineInput,
    PaginationParams, ResultTableInput, SetDataLinkStatus, StorageType,
    storage::memory::MemoryDataLinkRepository,
};

fn apply_spec(
    name: &str,
    domain: &str,
    owner_service: &str,
    data_type: DataType,
    status: DataLinkStatus,
    storage_type: StorageType,
    result_table_name: &str,
) -> ApplyDataLinkSpec {
    ApplyDataLinkSpec {
        name: name.to_string(),
        description: Some(format!("description-{name}")),
        domain: domain.to_string(),
        owner_service: owner_service.to_string(),
        data_type: data_type.clone(),
        status,
        status_message: None,
        datasource: DataSourceInput {
            producer: format!("producer-{name}"),
            data_type,
            collect_method: CollectMethod::Pull,
            protocol: Some("http_poll".to_string()),
            interval_seconds: Some(60),
            labels: HashMap::from([("domain".to_string(), domain.to_string())]),
            dimension_keys: vec!["tenant_id".to_string(), "record_id".to_string()],
            auth_ref: Some("secret://token".to_string()),
            config: HashMap::from([(
                "endpoint".to_string(),
                format!("https://api.example.com/{name}"),
            )]),
        },
        etl_pipeline: EtlPipelineInput {
            mode: EtlMode::Vector,
            config: HashMap::from([("mapping".to_string(), "k:v".to_string())]),
        },
        result_table: ResultTableInput {
            result_table_name: result_table_name.to_string(),
            storage_type,
            storage_cluster: Some("cluster-a".to_string()),
            database: Some("analytics".to_string()),
            table_name: Some(result_table_name.to_string()),
            metric_name: None,
            query_template: Some(format!("select * from {result_table_name}")),
            schema: HashMap::from([("record_id".to_string(), "string".to_string())]),
            retention_days: Some(30),
        },
    }
}

#[test]
fn apply_then_get_by_id_returns_full_bundle() {
    let repo = MemoryDataLinkRepository::new();
    let created = repo
        .apply(
            apply_spec(
                "wechat_ingest",
                "message",
                "im-gateway",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "dwd_wechat_messages",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-apply-get".to_string()),
            },
        )
        .expect("apply should succeed");

    let loaded = repo
        .get_by_id(&created.data_link.data_link_id)
        .expect("lookup should succeed")
        .expect("bundle should exist");

    assert_eq!(loaded, created);
    assert_eq!(loaded.data_link.result_table_name, "dwd_wechat_messages");
    assert_eq!(loaded.datasource.producer, "producer-wechat_ingest");
}

#[test]
fn get_by_result_table_name_works() {
    let repo = MemoryDataLinkRepository::new();
    let created = repo
        .apply(
            apply_spec(
                "audit_ingest",
                "security",
                "audit-gateway",
                DataType::Event,
                DataLinkStatus::Active,
                StorageType::Victoriametrics,
                "dwd_security_audit_events",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-by-table".to_string()),
            },
        )
        .expect("apply should succeed");

    let by_name = repo
        .get_by_result_table_name("dwd_security_audit_events")
        .expect("lookup should succeed")
        .expect("bundle should exist");

    assert_eq!(
        by_name.data_link.data_link_id,
        created.data_link.data_link_id
    );
    assert_eq!(
        by_name.result_table.result_table_name,
        "dwd_security_audit_events"
    );
}

#[test]
fn list_pagination_returns_deterministic_newest_first_order() {
    let repo = MemoryDataLinkRepository::new();

    let first = repo
        .apply(
            apply_spec(
                "first",
                "ops",
                "svc-a",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "table_first",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-order-1".to_string()),
            },
        )
        .unwrap();

    let second = repo
        .apply(
            apply_spec(
                "second",
                "ops",
                "svc-a",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "table_second",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-order-2".to_string()),
            },
        )
        .unwrap();

    let third = repo
        .apply(
            apply_spec(
                "third",
                "ops",
                "svc-a",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "table_third",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-order-3".to_string()),
            },
        )
        .unwrap();

    let page1 = repo
        .list(
            DataLinkListFilter {
                domain: None,
                owner_service: None,
                data_type: None,
                status: None,
                storage_type: None,
            },
            PaginationParams::new(1, 2),
        )
        .expect("page1 should succeed");
    let page2 = repo
        .list(
            DataLinkListFilter {
                domain: None,
                owner_service: None,
                data_type: None,
                status: None,
                storage_type: None,
            },
            PaginationParams::new(2, 2),
        )
        .expect("page2 should succeed");

    assert_eq!(page1.total, 3);
    assert_eq!(page1.items.len(), 2);
    assert_eq!(
        page1.items[0].data_link.data_link_id,
        third.data_link.data_link_id
    );
    assert_eq!(
        page1.items[1].data_link.data_link_id,
        second.data_link.data_link_id
    );

    assert_eq!(page2.total, 3);
    assert_eq!(page2.items.len(), 1);
    assert_eq!(
        page2.items[0].data_link.data_link_id,
        first.data_link.data_link_id
    );
}

#[test]
fn list_filtering_supports_domain_owner_data_type_status_and_storage_type() {
    let repo = MemoryDataLinkRepository::new();

    let target = repo
        .apply(
            apply_spec(
                "target",
                "security",
                "audit-gateway",
                DataType::Event,
                DataLinkStatus::Active,
                StorageType::Victoriametrics,
                "security_target",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-filter-target".to_string()),
            },
        )
        .unwrap();

    repo.apply(
        apply_spec(
            "other-domain",
            "message",
            "audit-gateway",
            DataType::Event,
            DataLinkStatus::Active,
            StorageType::Victoriametrics,
            "security_other_domain",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-filter-1".to_string()),
        },
    )
    .unwrap();

    repo.apply(
        apply_spec(
            "other-owner",
            "security",
            "im-gateway",
            DataType::Event,
            DataLinkStatus::Active,
            StorageType::Victoriametrics,
            "security_other_owner",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-filter-2".to_string()),
        },
    )
    .unwrap();

    repo.apply(
        apply_spec(
            "other-data-type",
            "security",
            "audit-gateway",
            DataType::Log,
            DataLinkStatus::Active,
            StorageType::Victoriametrics,
            "security_other_data_type",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-filter-3".to_string()),
        },
    )
    .unwrap();

    repo.apply(
        apply_spec(
            "other-status",
            "security",
            "audit-gateway",
            DataType::Event,
            DataLinkStatus::Disabled,
            StorageType::Victoriametrics,
            "security_other_status",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-filter-4".to_string()),
        },
    )
    .unwrap();

    repo.apply(
        apply_spec(
            "other-storage",
            "security",
            "audit-gateway",
            DataType::Event,
            DataLinkStatus::Active,
            StorageType::Mysql,
            "security_other_storage",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-filter-5".to_string()),
        },
    )
    .unwrap();

    let filtered = repo
        .list(
            DataLinkListFilter {
                domain: Some("security".to_string()),
                owner_service: Some("audit-gateway".to_string()),
                data_type: Some(DataType::Event),
                status: Some(DataLinkStatus::Active),
                storage_type: Some(StorageType::Victoriametrics),
            },
            PaginationParams::new(1, 20),
        )
        .expect("filter list should succeed");

    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(
        filtered.items[0].data_link.data_link_id,
        target.data_link.data_link_id
    );
}

#[test]
fn set_status_updates_status_and_status_message() {
    let repo = MemoryDataLinkRepository::new();
    let created = repo
        .apply(
            apply_spec(
                "status_case",
                "ops",
                "status-svc",
                DataType::Metric,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "status_case_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-status".to_string()),
            },
        )
        .unwrap();

    let updated = repo
        .set_status(
            &created.data_link.data_link_id,
            SetDataLinkStatus {
                status: DataLinkStatus::Disabled,
                reason: Some("maintenance window".to_string()),
                status_message: None,
            },
        )
        .expect("set status should succeed");

    assert_eq!(updated.data_link.status, DataLinkStatus::Disabled);
    assert_eq!(
        updated.data_link.status_message.as_deref(),
        Some("maintenance window")
    );
}

#[test]
fn repeated_identical_apply_uses_same_logical_identity() {
    let repo = MemoryDataLinkRepository::new();
    let spec = apply_spec(
        "same_identity",
        "message",
        "im-gateway",
        DataType::Log,
        DataLinkStatus::Active,
        StorageType::Mysql,
        "same_identity_table",
    );

    let first = repo
        .apply(
            spec.clone(),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect("first apply should succeed");

    let second = repo
        .apply(
            spec,
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect("second apply should succeed");

    assert_eq!(first.data_link.data_link_id, second.data_link.data_link_id);
    assert_eq!(
        first.datasource.datasource_id,
        second.datasource.datasource_id
    );
    assert_eq!(
        first.etl_pipeline.etl_pipeline_id,
        second.etl_pipeline.etl_pipeline_id
    );
    assert_eq!(
        first.result_table.result_table_id,
        second.result_table.result_table_id
    );
}

#[test]
fn apply_updates_mutable_fields_and_preserves_result_table_name_uniqueness() {
    let repo = MemoryDataLinkRepository::new();
    let first = repo
        .apply(
            apply_spec(
                "mutable",
                "message",
                "im-gateway",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "mutable_table_v1",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-mutable".to_string()),
            },
        )
        .unwrap();

    let mut updated_spec = apply_spec(
        "mutable",
        "message",
        "im-gateway",
        DataType::Trace,
        DataLinkStatus::Active,
        StorageType::Mysql,
        "mutable_table_v2",
    );
    updated_spec.description = Some("updated description".to_string());
    updated_spec.datasource.producer = "producer-mutable-updated".to_string();
    updated_spec.datasource.config.insert(
        "endpoint".to_string(),
        "https://api.example.com/mutable-updated".to_string(),
    );

    let updated = repo
        .apply(
            updated_spec,
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-mutable".to_string()),
            },
        )
        .expect("update apply should succeed");

    assert_eq!(updated.data_link.data_link_id, first.data_link.data_link_id);
    assert_eq!(
        updated.data_link.description.as_deref(),
        Some("updated description")
    );
    assert_eq!(updated.data_link.data_type, DataType::Trace);
    assert_eq!(updated.data_link.status, DataLinkStatus::Active);
    assert_eq!(updated.datasource.producer, "producer-mutable-updated");
    assert_eq!(
        updated
            .datasource
            .config
            .get("endpoint")
            .map(String::as_str),
        Some("https://api.example.com/mutable-updated")
    );
    assert_eq!(updated.result_table.result_table_name, "mutable_table_v2");

    let old_table = repo
        .get_by_result_table_name("mutable_table_v1")
        .expect("lookup should succeed");
    assert!(
        old_table.is_none(),
        "old index should be removed after rename"
    );

    repo.apply(
        apply_spec(
            "other-link",
            "message",
            "im-gateway",
            DataType::Log,
            DataLinkStatus::Draft,
            StorageType::Mysql,
            "other_table",
        ),
        ApplyDataLinkOptions {
            idempotency_key: Some("idem-other".to_string()),
        },
    )
    .unwrap();

    let conflict = repo
        .apply(
            apply_spec(
                "mutable",
                "message",
                "im-gateway",
                DataType::Trace,
                DataLinkStatus::Active,
                StorageType::Mysql,
                "other_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-mutable".to_string()),
            },
        )
        .expect_err("result_table_name conflict should be returned");

    assert!(matches!(
        conflict,
        DataLinkError::ResultTableNameConflict(_)
    ));
}

#[test]
fn changing_logical_identity_removes_stale_aliases() {
    let repo = MemoryDataLinkRepository::new();

    let first = repo
        .apply(
            apply_spec(
                "alias-base",
                "domain-a",
                "svc-a",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "alias_base_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-alias".to_string()),
            },
        )
        .unwrap();

    let moved = repo
        .apply(
            apply_spec(
                "alias-moved",
                "domain-b",
                "svc-b",
                DataType::Trace,
                DataLinkStatus::Active,
                StorageType::Mysql,
                "alias_moved_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-alias".to_string()),
            },
        )
        .unwrap();

    assert_eq!(first.data_link.data_link_id, moved.data_link.data_link_id);

    let reused_old_logical = repo
        .apply(
            apply_spec(
                "alias-base",
                "domain-a",
                "svc-a",
                DataType::Event,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "alias_old_logical_reuse",
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .unwrap();

    assert_ne!(
        reused_old_logical.data_link.data_link_id, moved.data_link.data_link_id,
        "old logical identity must not keep stale alias mapping"
    );
}

#[test]
fn rotating_idempotency_key_removes_old_key_alias() {
    let repo = MemoryDataLinkRepository::new();

    let first = repo
        .apply(
            apply_spec(
                "key-rotation",
                "security",
                "svc-security",
                DataType::Event,
                DataLinkStatus::Draft,
                StorageType::Victoriametrics,
                "key_rotation_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-old-key".to_string()),
            },
        )
        .unwrap();

    let rotated = repo
        .apply(
            apply_spec(
                "key-rotation",
                "security",
                "svc-security",
                DataType::Event,
                DataLinkStatus::Active,
                StorageType::Victoriametrics,
                "key_rotation_table_v2",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-new-key".to_string()),
            },
        )
        .unwrap();

    assert_eq!(first.data_link.data_link_id, rotated.data_link.data_link_id);

    let old_key_reused = repo
        .apply(
            apply_spec(
                "new-link-on-old-key",
                "other-domain",
                "other-svc",
                DataType::Log,
                DataLinkStatus::Draft,
                StorageType::Mysql,
                "reused_old_key_new_table",
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("idem-old-key".to_string()),
            },
        )
        .unwrap();

    assert_ne!(
        old_key_reused.data_link.data_link_id, rotated.data_link.data_link_id,
        "old idempotency key should not remain as stale alias"
    );
}
