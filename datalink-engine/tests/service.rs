use std::{collections::HashMap, sync::Mutex};

use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLinkBundle, DataLinkError,
    DataLinkListFilter, DataLinkRepository, DataLinkService, DataLinkStatus, DataSourceInput,
    DataType, EtlMode, EtlPipelineInput, PaginatedResult, PaginationParams, ResultTableInput,
    SetDataLinkStatus, StorageType, storage::memory::MemoryDataLinkRepository,
};
use serde_json::json;

fn spec(
    name: &str,
    domain: &str,
    owner_service: &str,
    result_table_name: &str,
    status: DataLinkStatus,
    status_message: Option<&str>,
    mode: EtlMode,
    etl_config: HashMap<String, String>,
) -> ApplyDataLinkSpec {
    ApplyDataLinkSpec {
        name: name.to_string(),
        description: Some(format!("desc-{name}")),
        domain: domain.to_string(),
        owner_service: owner_service.to_string(),
        data_type: DataType::Log,
        status,
        status_message: status_message.map(str::to_string),
        datasource: DataSourceInput {
            producer: format!("producer-{name}"),
            data_type: DataType::Log,
            collect_method: CollectMethod::Pull,
            protocol: Some("http_poll".to_string()),
            interval_seconds: Some(60),
            labels: HashMap::from([("domain".to_string(), domain.to_string())]),
            dimension_keys: vec!["tenant_id".to_string()],
            auth_ref: Some("secret://token".to_string()),
            config: HashMap::from([(
                "endpoint".to_string(),
                format!("https://api.example.com/{name}"),
            )]),
        },
        etl_pipeline: EtlPipelineInput {
            mode,
            config: etl_config,
        },
        result_table: ResultTableInput {
            result_table_name: result_table_name.to_string(),
            storage_type: StorageType::Mysql,
            storage_cluster: Some("cluster-a".to_string()),
            database: Some("analytics".to_string()),
            table_name: Some(result_table_name.to_string()),
            metric_name: None,
            query_template: Some(format!("select * from {result_table_name}")),
            schema: HashMap::from([("id".to_string(), "string".to_string())]),
            retention_days: Some(7),
        },
    }
}

#[derive(Debug, Default)]
struct FailFirstApplyRepository {
    inner: MemoryDataLinkRepository,
    fail_next: Mutex<bool>,
}

#[derive(Debug, Default)]
struct SaveFailureRepository {
    inner: MemoryDataLinkRepository,
}

impl SaveFailureRepository {
    fn new() -> Self {
        Self {
            inner: MemoryDataLinkRepository::new(),
        }
    }
}

impl FailFirstApplyRepository {
    fn new() -> Self {
        Self {
            inner: MemoryDataLinkRepository::new(),
            fail_next: Mutex::new(true),
        }
    }
}

impl DataLinkRepository for FailFirstApplyRepository {
    fn apply(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        let mut fail_next = self
            .fail_next
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        if *fail_next {
            *fail_next = false;
            return Err(DataLinkError::Repository(
                "transient apply failure".to_string(),
            ));
        }

        self.inner.apply(spec, options)
    }

    fn apply_with_idempotency(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        let mut fail_next = self
            .fail_next
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        if *fail_next {
            *fail_next = false;
            return Err(DataLinkError::Repository(
                "transient apply failure".to_string(),
            ));
        }

        self.inner.apply_with_idempotency(spec, options)
    }

    fn get_by_id(
        &self,
        data_link_id: &str,
    ) -> datalink_engine::error::Result<Option<DataLinkBundle>> {
        self.inner.get_by_id(data_link_id)
    }

    fn get_by_result_table_name(
        &self,
        result_table_name: &str,
    ) -> datalink_engine::error::Result<Option<DataLinkBundle>> {
        self.inner.get_by_result_table_name(result_table_name)
    }

    fn list(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> datalink_engine::error::Result<PaginatedResult<DataLinkBundle>> {
        self.inner.list(filter, pagination)
    }

    fn set_status(
        &self,
        data_link_id: &str,
        request: SetDataLinkStatus,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        self.inner.set_status(data_link_id, request)
    }
}

impl DataLinkRepository for SaveFailureRepository {
    fn apply(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        self.inner.apply(spec, options)
    }

    fn apply_with_idempotency(
        &self,
        _spec: ApplyDataLinkSpec,
        _options: ApplyDataLinkOptions,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        Err(DataLinkError::Repository(
            "simulated idempotency persistence failure".to_string(),
        ))
    }

    fn get_by_id(
        &self,
        data_link_id: &str,
    ) -> datalink_engine::error::Result<Option<DataLinkBundle>> {
        self.inner.get_by_id(data_link_id)
    }

    fn get_by_result_table_name(
        &self,
        result_table_name: &str,
    ) -> datalink_engine::error::Result<Option<DataLinkBundle>> {
        self.inner.get_by_result_table_name(result_table_name)
    }

    fn list(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> datalink_engine::error::Result<PaginatedResult<DataLinkBundle>> {
        self.inner.list(filter, pagination)
    }

    fn set_status(
        &self,
        data_link_id: &str,
        request: SetDataLinkStatus,
    ) -> datalink_engine::error::Result<DataLinkBundle> {
        self.inner.set_status(data_link_id, request)
    }
}

#[test]
fn apply_data_link_rejects_duplicate_result_table_name() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());

    service
        .apply_data_link(
            spec(
                "a",
                "message",
                "svc-a",
                "dup_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("k-a".to_string()),
            },
        )
        .unwrap();

    let err = service
        .apply_data_link(
            spec(
                "b",
                "message",
                "svc-b",
                "dup_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("k-b".to_string()),
            },
        )
        .expect_err("duplicate result_table_name should be rejected");

    assert!(matches!(err, DataLinkError::ResultTableNameConflict(_)));
}

#[test]
fn apply_data_link_is_idempotent_for_repeated_equivalent_requests() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let apply = || {
        service.apply_data_link(
            spec(
                "idem",
                "message",
                "svc",
                "idem_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
    };

    let first = apply().unwrap();
    let second = apply().unwrap();

    assert_eq!(first.data_link.data_link_id, second.data_link.data_link_id);
}

#[test]
fn apply_data_link_honors_idempotency_key_priority() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    service
        .apply_data_link(
            spec(
                "a",
                "message",
                "svc-a",
                "table_a",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("x-idem-priority".to_string()),
            },
        )
        .unwrap();

    let err = service
        .apply_data_link(
            spec(
                "different-logical-link",
                "security",
                "svc-b",
                "table_b",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k2:v2".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("x-idem-priority".to_string()),
            },
        )
        .expect_err("same idempotency key with different body must conflict");

    assert!(matches!(err, DataLinkError::IdempotencyConflict(_)));
}

#[test]
fn same_idempotency_key_plus_same_body_returns_same_logical_result() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let first = service
        .apply_data_link(
            spec(
                "same-body",
                "message",
                "svc",
                "same_body_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("same-body-key".to_string()),
            },
        )
        .unwrap();

    let second = service
        .apply_data_link(
            spec(
                "same-body",
                "message",
                "svc",
                "same_body_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("same-body-key".to_string()),
            },
        )
        .unwrap();

    assert_eq!(first.data_link.data_link_id, second.data_link.data_link_id);
}

#[test]
fn same_idempotency_key_plus_different_body_returns_idempotency_conflict() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    service
        .apply_data_link(
            spec(
                "k-conflict",
                "message",
                "svc",
                "conflict_table_a",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("key-conflict".to_string()),
            },
        )
        .unwrap();

    let err = service
        .apply_data_link(
            spec(
                "k-conflict",
                "message",
                "svc",
                "conflict_table_b",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:changed".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("key-conflict".to_string()),
            },
        )
        .expect_err("same key + different payload must fail");

    assert!(matches!(err, DataLinkError::IdempotencyConflict(_)));
}

#[test]
fn transient_repository_failure_does_not_poison_idempotency_key() {
    let service = DataLinkService::new(FailFirstApplyRepository::new());

    let first_err = service
        .apply_data_link(
            spec(
                "poison-check-a",
                "message",
                "svc",
                "poison_table_a",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("poison-key".to_string()),
            },
        )
        .expect_err("first apply should fail transiently");
    assert!(matches!(first_err, DataLinkError::Repository(_)));

    let retried = service
        .apply_data_link(
            spec(
                "poison-check-b",
                "message",
                "svc",
                "poison_table_b",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:changed".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("poison-key".to_string()),
            },
        )
        .expect("failed apply must not poison idempotency key");

    assert_eq!(retried.data_link.name, "poison-check-b");
    assert_eq!(retried.result_table.result_table_name, "poison_table_b");
}

#[test]
fn idempotency_persistence_failure_does_not_leave_created_link_behind() {
    let service = DataLinkService::new(SaveFailureRepository::new());

    let err = service
        .apply_data_link(
            spec(
                "atomicity-check",
                "message",
                "svc",
                "atomicity_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("atomicity-key".to_string()),
            },
        )
        .expect_err("idempotency persistence failure should fail request");

    assert!(matches!(err, DataLinkError::Repository(_)));

    let loaded = service.get_data_link_by_result_table_name("atomicity_table");
    assert!(
        matches!(loaded, Err(DataLinkError::NotFound(_))),
        "failed apply must not leave a created link behind"
    );
}

#[test]
fn replaying_old_idempotency_key_does_not_overwrite_newer_state() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());

    let original = service
        .apply_data_link(
            spec(
                "stable-idem",
                "message",
                "svc",
                "stable_idem_v1",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "v1".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("stable-key-v1".to_string()),
            },
        )
        .unwrap();

    let updated = service
        .apply_data_link(
            spec(
                "stable-idem",
                "message",
                "svc",
                "stable_idem_v2",
                DataLinkStatus::Active,
                Some("will clear"),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "v2".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("stable-key-v2".to_string()),
            },
        )
        .unwrap();

    assert_eq!(
        original.data_link.data_link_id,
        updated.data_link.data_link_id
    );
    assert_eq!(updated.result_table.result_table_name, "stable_idem_v2");

    let replay = service
        .apply_data_link(
            spec(
                "stable-idem",
                "message",
                "svc",
                "stable_idem_v1",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "v1".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: Some("stable-key-v1".to_string()),
            },
        )
        .unwrap();

    assert_eq!(
        replay, original,
        "idempotent replay must return original committed result"
    );

    let current = service
        .get_data_link(&updated.data_link.data_link_id)
        .unwrap();
    assert_eq!(current.result_table.result_table_name, "stable_idem_v2");
    assert_eq!(
        current
            .etl_pipeline
            .config
            .get("mapping")
            .map(String::as_str),
        Some("v2"),
        "replaying the old idempotency key must not overwrite newer state"
    );
}

#[test]
fn apply_data_link_updates_mutable_fields_instead_of_creating_duplicates() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let first = service
        .apply_data_link(
            spec(
                "mutable",
                "message",
                "svc",
                "mutable_v1",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .unwrap();

    let second = service
        .apply_data_link(
            spec(
                "mutable",
                "message",
                "svc",
                "mutable_v2",
                DataLinkStatus::Active,
                Some("should be cleared"),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:updated".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .unwrap();

    assert_eq!(first.data_link.data_link_id, second.data_link.data_link_id);
    assert_eq!(second.result_table.result_table_name, "mutable_v2");
    assert_eq!(
        second
            .etl_pipeline
            .config
            .get("mapping")
            .map(String::as_str),
        Some("k:updated")
    );
}

#[test]
fn mode_passthrough_accepts_empty_config() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let created = service
        .apply_data_link(
            spec(
                "passthrough",
                "message",
                "svc",
                "passthrough_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Passthrough,
                HashMap::new(),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect("passthrough with empty config should be accepted");

    assert_eq!(created.etl_pipeline.mode, EtlMode::Passthrough);
}

#[test]
fn mode_vector_accepts_non_empty_config_and_invalid_mode_is_rejected() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());

    let created = service
        .apply_data_link(
            spec(
                "vector",
                "message",
                "svc",
                "vector_table",
                DataLinkStatus::Draft,
                None,
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "a:b".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect("vector with non-empty config should be accepted");
    assert_eq!(created.etl_pipeline.mode, EtlMode::Vector);

    let invalid = serde_json::from_value::<ApplyDataLinkSpec>(json!({
        "name": "invalid-mode",
        "domain": "message",
        "owner_service": "svc",
        "data_type": "log",
        "status": "draft",
        "datasource": {
            "producer": "producer",
            "data_type": "log",
            "collect_method": "pull",
            "dimension_keys": [],
            "config": {}
        },
        "etl_pipeline": {
            "mode": "unsupported-mode",
            "config": {}
        },
        "result_table": {
            "result_table_name": "rt_invalid_mode",
            "storage_type": "mysql",
            "schema": {}
        }
    }));

    assert!(
        invalid.is_err(),
        "invalid mode should be rejected at input boundary"
    );
}

#[test]
fn unknown_legacy_etl_fields_are_rejected_at_input_boundary() {
    for field in ["metadata", "steps", "pipeline_name", "version", "enabled"] {
        let payload = json!({
            "name": "legacy-field",
            "domain": "message",
            "owner_service": "svc",
            "data_type": "log",
            "status": "draft",
            "datasource": {
                "producer": "producer",
                "data_type": "log",
                "collect_method": "pull",
                "dimension_keys": [],
                "config": {}
            },
            "etl_pipeline": {
                "mode": "vector",
                "config": {},
                field: "legacy"
            },
            "result_table": {
                "result_table_name": "rt_legacy",
                "storage_type": "mysql",
                "schema": {}
            }
        });

        let err = serde_json::from_value::<ApplyDataLinkSpec>(payload)
            .expect_err("legacy etl_pipeline fields must be rejected");
        assert!(
            err.to_string().contains(field),
            "error should mention unknown field {field}: {err}"
        );
    }
}

#[test]
fn status_active_clears_status_message() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let created = service
        .apply_data_link(
            spec(
                "active",
                "message",
                "svc",
                "active_table",
                DataLinkStatus::Active,
                Some("should clear"),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect("active status should be accepted");

    assert_eq!(created.data_link.status, DataLinkStatus::Active);
    assert_eq!(created.data_link.status_message, None);
}

#[test]
fn status_disabled_requires_non_empty_status_message_or_reason() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());

    let err = service
        .apply_data_link(
            spec(
                "disabled",
                "message",
                "svc",
                "disabled_table",
                DataLinkStatus::Disabled,
                Some("   "),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .expect_err("disabled without non-empty status_message should be rejected");
    assert!(matches!(err, DataLinkError::StatusMessageRequired));

    let created = service
        .apply_data_link(
            spec(
                "disabled-ok",
                "message",
                "svc",
                "disabled_ok_table",
                DataLinkStatus::Disabled,
                Some("maintenance"),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .unwrap();

    let disabled_ok = service
        .set_data_link_status(
            &created.data_link.data_link_id,
            SetDataLinkStatus {
                status: DataLinkStatus::Disabled,
                reason: Some("manual pause".to_string()),
                status_message: None,
            },
        )
        .expect("disabled with reason should be accepted");
    assert_eq!(
        disabled_ok.data_link.status_message.as_deref(),
        Some("manual pause")
    );
}

#[test]
fn illegal_status_transition_deleted_to_active_is_rejected() {
    let service = DataLinkService::new(MemoryDataLinkRepository::new());
    let created = service
        .apply_data_link(
            spec(
                "deleted-link",
                "message",
                "svc",
                "deleted_link_table",
                DataLinkStatus::Deleted,
                Some("retired"),
                EtlMode::Vector,
                HashMap::from([("mapping".to_string(), "k:v".to_string())]),
            ),
            ApplyDataLinkOptions {
                idempotency_key: None,
            },
        )
        .unwrap();

    let err = service
        .set_data_link_status(
            &created.data_link.data_link_id,
            SetDataLinkStatus {
                status: DataLinkStatus::Active,
                reason: None,
                status_message: Some("reactivate".to_string()),
            },
        )
        .expect_err("deleted -> active transition should be rejected");

    assert!(matches!(err, DataLinkError::StatusTransitionInvalid { .. }));
}
