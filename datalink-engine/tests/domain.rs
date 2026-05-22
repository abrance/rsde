use std::collections::HashMap;

use chrono::Utc;
use datalink_engine::{
    ApplyDataLinkOptions, ApplyDataLinkSpec, CollectMethod, DataLink, DataLinkBundle,
    DataLinkError, DataLinkListFilter, DataLinkStatus, DataSource, DataType, EtlMode, EtlPipeline,
    PaginatedResult, PaginationParams, ResultTable, StorageType,
};
use serde_json::json;

#[test]
fn apply_datalink_spec_deserializes_v1_payload() {
    let payload = json!({
        "name": "wechat_message_ingest",
        "description": "collect and store raw message records",
        "domain": "message",
        "owner_service": "im-gateway",
        "data_type": "log",
        "status": "active",
        "status_message": null,
        "datasource": {
            "producer": "wechat_api",
            "data_type": "log",
            "collect_method": "pull",
            "protocol": "http_poll",
            "interval_seconds": 60,
            "labels": {
                "domain": "message"
            },
            "dimension_keys": ["tenant_id", "msg_id"],
            "auth_ref": "secret://wechat/token",
            "config": {
                "endpoint": "https://api.example.com/messages",
                "token": "masked"
            }
        },
        "etl_pipeline": {
            "mode": "vector",
            "config": {
                "mapping": "msg_id:id,sender:from"
            }
        },
        "result_table": {
            "storage_type": "mysql",
            "result_table_name": "dwd_wechat_messages",
            "storage_cluster": "mysql-primary",
            "database": "message",
            "table_name": "dwd_wechat_messages",
            "metric_name": null,
            "query_template": "select * from dwd_wechat_messages where tenant_id = ?",
            "schema": {
                "msg_id": "string",
                "sender": "string"
            },
            "retention_days": 30
        }
    });

    let spec: ApplyDataLinkSpec = serde_json::from_value(payload).expect("valid v1 payload");

    assert_eq!(spec.name, "wechat_message_ingest");
    assert_eq!(spec.domain, "message");
    assert_eq!(spec.owner_service, "im-gateway");
    assert_eq!(spec.data_type, DataType::Log);
    assert_eq!(spec.status, DataLinkStatus::Active);
    assert_eq!(spec.datasource.data_type, DataType::Log);
    assert_eq!(spec.datasource.collect_method, CollectMethod::Pull);
    assert_eq!(spec.etl_pipeline.mode, EtlMode::Vector);
    assert_eq!(spec.result_table.storage_type, StorageType::Mysql);
    assert_eq!(
        spec.etl_pipeline.config.get("mapping").map(String::as_str),
        Some("msg_id:id,sender:from")
    );
    assert_eq!(
        spec.datasource.config.get("endpoint").map(String::as_str),
        Some("https://api.example.com/messages")
    );
    assert_eq!(
        spec.result_table.table_name.as_deref(),
        Some("dwd_wechat_messages")
    );
}

#[test]
fn apply_datalink_spec_requires_etl_pipeline() {
    let payload = json!({
        "name": "wechat_message_ingest",
        "domain": "message",
        "owner_service": "im-gateway",
        "data_type": "log",
        "status": "draft",
        "datasource": {
            "producer": "wechat_api",
            "data_type": "log",
            "collect_method": "pull",
            "dimension_keys": [],
            "config": {
                "endpoint": "https://api.example.com/messages"
            }
        },
        "result_table": {
            "storage_type": "mysql",
            "result_table_name": "dwd_wechat_messages",
            "schema": {}
        }
    });

    let err = serde_json::from_value::<ApplyDataLinkSpec>(payload)
        .expect_err("missing etl_pipeline should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("etl_pipeline"),
        "unexpected error for missing etl_pipeline: {message}"
    );
}

#[test]
fn apply_datalink_spec_rejects_legacy_fields_in_etl_pipeline() {
    let legacy_fields = ["metadata", "steps", "pipeline_name", "version", "enabled"];

    for field in legacy_fields {
        let payload = json!({
            "name": "legacy_input",
            "domain": "message",
            "owner_service": "im-gateway",
            "data_type": "log",
            "status": "draft",
            "datasource": {
                "producer": "wechat_api",
                "data_type": "log",
                "collect_method": "pull",
                "dimension_keys": [],
                "config": {}
            },
            "etl_pipeline": {
                "mode": "vector",
                "config": {},
                field: true
            },
            "result_table": {
                "storage_type": "mysql",
                "result_table_name": "table_a",
                "schema": {}
            }
        });

        let err = serde_json::from_value::<ApplyDataLinkSpec>(payload)
            .expect_err("legacy field in etl_pipeline must be rejected");
        let message = err.to_string();
        assert!(
            message.contains(field),
            "unexpected unknown-field error for {field}: {message}"
        );
    }
}

#[test]
fn apply_datalink_options_supports_idempotency_key() {
    let opts: ApplyDataLinkOptions = serde_json::from_value(json!({
        "idempotency_key": "idem-001"
    }))
    .expect("idempotency key should deserialize");

    assert_eq!(opts.idempotency_key.as_deref(), Some("idem-001"));
}

#[test]
fn enums_follow_v1_contract_values() {
    assert_eq!(
        serde_json::from_value::<DataType>(json!("metric")).unwrap(),
        DataType::Metric
    );
    assert_eq!(
        serde_json::from_value::<DataType>(json!("log")).unwrap(),
        DataType::Log
    );
    assert_eq!(
        serde_json::from_value::<DataType>(json!("trace")).unwrap(),
        DataType::Trace
    );
    assert_eq!(
        serde_json::from_value::<DataType>(json!("event")).unwrap(),
        DataType::Event
    );
    assert_eq!(
        serde_json::from_value::<DataType>(json!("profile")).unwrap(),
        DataType::Profile
    );

    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("agent")).unwrap(),
        CollectMethod::Agent
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("push")).unwrap(),
        CollectMethod::Push
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("pull")).unwrap(),
        CollectMethod::Pull
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("ebpf")).unwrap(),
        CollectMethod::Ebpf
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("file")).unwrap(),
        CollectMethod::File
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("http")).unwrap(),
        CollectMethod::Http
    );
    assert_eq!(
        serde_json::from_value::<CollectMethod>(json!("kafka")).unwrap(),
        CollectMethod::Kafka
    );

    assert_eq!(
        serde_json::from_value::<DataLinkStatus>(json!("draft")).unwrap(),
        DataLinkStatus::Draft
    );
    assert_eq!(
        serde_json::from_value::<DataLinkStatus>(json!("active")).unwrap(),
        DataLinkStatus::Active
    );
    assert_eq!(
        serde_json::from_value::<DataLinkStatus>(json!("disabled")).unwrap(),
        DataLinkStatus::Disabled
    );
    assert_eq!(
        serde_json::from_value::<DataLinkStatus>(json!("deleted")).unwrap(),
        DataLinkStatus::Deleted
    );

    assert_eq!(
        serde_json::from_value::<EtlMode>(json!("passthrough")).unwrap(),
        EtlMode::Passthrough
    );
    assert_eq!(
        serde_json::from_value::<EtlMode>(json!("vector")).unwrap(),
        EtlMode::Vector
    );

    assert_eq!(
        serde_json::from_value::<StorageType>(json!("victoriametrics")).unwrap(),
        StorageType::Victoriametrics
    );
    assert_eq!(
        serde_json::from_value::<StorageType>(json!("mysql")).unwrap(),
        StorageType::Mysql
    );

    assert!(serde_json::from_value::<DataType>(json!("api")).is_err());
    assert!(serde_json::from_value::<CollectMethod>(json!("incremental")).is_err());
    assert!(serde_json::from_value::<DataLinkStatus>(json!("inactive")).is_err());
    assert!(serde_json::from_value::<EtlMode>(json!("copy")).is_err());
    assert!(serde_json::from_value::<StorageType>(json!("database")).is_err());
}

#[test]
fn bundle_and_pagination_domain_models_work() {
    let now = Utc::now();
    let data_link = DataLink {
        data_link_id: "dl_001".to_string(),
        name: "wechat_message_ingest".to_string(),
        description: Some("demo".to_string()),
        domain: "message".to_string(),
        owner_service: "im-gateway".to_string(),
        data_type: DataType::Log,
        datasource_id: "ds_001".to_string(),
        etl_pipeline_id: "etl_001".to_string(),
        result_table_id: "rt_001".to_string(),
        result_table_name: "dwd_wechat_messages".to_string(),
        status: DataLinkStatus::Active,
        status_message: None,
        created_at: now,
        updated_at: now,
    };

    let bundle = DataLinkBundle {
        data_link,
        datasource: DataSource {
            datasource_id: "ds_001".to_string(),
            data_link_id: "dl_001".to_string(),
            producer: "wechat_api".to_string(),
            data_type: DataType::Log,
            collect_method: CollectMethod::Pull,
            protocol: Some("http_poll".to_string()),
            interval_seconds: Some(60),
            labels: HashMap::from([("domain".to_string(), "message".to_string())]),
            dimension_keys: vec!["tenant_id".to_string(), "msg_id".to_string()],
            auth_ref: Some("secret://wechat/token".to_string()),
            config: HashMap::from([(
                "endpoint".to_string(),
                "https://api.example.com/messages".to_string(),
            )]),
            created_at: now,
            updated_at: now,
        },
        etl_pipeline: EtlPipeline {
            etl_pipeline_id: "etl_001".to_string(),
            data_link_id: "dl_001".to_string(),
            mode: EtlMode::Vector,
            config: HashMap::from([("mapping".to_string(), "msg_id:id".to_string())]),
            created_at: now,
            updated_at: now,
        },
        result_table: ResultTable {
            result_table_id: "rt_001".to_string(),
            data_link_id: "dl_001".to_string(),
            result_table_name: "dwd_wechat_messages".to_string(),
            storage_type: StorageType::Mysql,
            storage_cluster: Some("mysql-primary".to_string()),
            database: Some("message".to_string()),
            table_name: Some("dwd_wechat_messages".to_string()),
            metric_name: None,
            query_template: Some("select * from dwd_wechat_messages".to_string()),
            schema: HashMap::from([
                ("msg_id".to_string(), "string".to_string()),
                ("sender".to_string(), "string".to_string()),
            ]),
            retention_days: Some(30),
            created_at: now,
            updated_at: now,
        },
    };

    assert_eq!(bundle.data_link.status, DataLinkStatus::Active);
    assert_eq!(bundle.etl_pipeline.mode, EtlMode::Vector);

    let params = PaginationParams::new(2, 50);
    assert_eq!(params.offset(), 50);
    assert_eq!(params.limit(), 50);

    let page = PaginatedResult::new(vec![bundle], 101, &params);
    assert_eq!(page.total_pages, 3);
    assert_eq!(page.page, 2);
    assert_eq!(page.page_size, 50);

    let _filter = DataLinkListFilter {
        domain: Some("message".to_string()),
        owner_service: Some("im-gateway".to_string()),
        data_type: Some(DataType::Log),
        status: Some(DataLinkStatus::Active),
        storage_type: Some(StorageType::Mysql),
    };
}

#[test]
fn datalink_error_variants_match_v1_contract() {
    let errors = vec![
        DataLinkError::InvalidArgument("bad request".to_string()),
        DataLinkError::EnumNotSupported("unsupported enum".to_string()),
        DataLinkError::ResultTableNameConflict("duplicate table".to_string()),
        DataLinkError::EtlPipelineInvalid("bad etl pipeline".to_string()),
        DataLinkError::EtlModeNotSupported("bad etl mode".to_string()),
        DataLinkError::NotFound("missing".to_string()),
        DataLinkError::StatusTransitionInvalid {
            from: "deleted".to_string(),
            to: "active".to_string(),
        },
        DataLinkError::StatusMessageRequired,
        DataLinkError::StatusMessageInvalid("blank".to_string()),
        DataLinkError::IdempotencyConflict("conflict".to_string()),
        DataLinkError::BackendNotSupported("sqlite".to_string()),
        DataLinkError::Repository("storage failed".to_string()),
    ];

    assert_eq!(errors.len(), 12);
}
