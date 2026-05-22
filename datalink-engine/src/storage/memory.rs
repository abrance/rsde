use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::Utc;

use crate::{
    DataLinkError, DataLinkRepository,
    error::Result,
    models::{
        ApplyDataLinkOptions, ApplyDataLinkSpec, DataLink, DataLinkBundle, DataLinkListFilter,
        DataSource, EtlPipeline, PaginatedResult, PaginationParams, ResultTable, SetDataLinkStatus,
        new_id,
    },
};

#[derive(Debug, Clone, Default)]
pub struct MemoryDataLinkRepository {
    state: Arc<Mutex<MemoryState>>,
}

#[derive(Debug, Clone, Default)]
struct MemoryState {
    bundles: HashMap<String, DataLinkBundle>,
    result_table_index: HashMap<String, String>,
    identity_index: HashMap<String, String>,
    idempotency_specs: HashMap<String, ApplyDataLinkSpec>,
    idempotency_results: HashMap<String, DataLinkBundle>,
    order_index: HashMap<String, u64>,
    next_order: u64,
}

impl MemoryDataLinkRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn identity_key(options: &ApplyDataLinkOptions, spec: &ApplyDataLinkSpec) -> String {
        if let Some(key) = options
            .idempotency_key
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            return format!("idempotency:{key}");
        }

        Self::fallback_identity_key(spec)
    }

    fn fallback_identity_key(spec: &ApplyDataLinkSpec) -> String {
        format!(
            "logical:{}|{}|{}",
            spec.domain, spec.owner_service, spec.name
        )
    }

    fn bump_order(state: &mut MemoryState, data_link_id: &str) {
        state.next_order = state.next_order.saturating_add(1);
        state
            .order_index
            .insert(data_link_id.to_string(), state.next_order);
    }

    fn create_bundle(spec: ApplyDataLinkSpec) -> DataLinkBundle {
        let now = Utc::now();
        let data_link_id = new_id();
        let datasource_id = new_id();
        let etl_pipeline_id = new_id();
        let result_table_id = new_id();

        DataLinkBundle {
            data_link: DataLink {
                data_link_id: data_link_id.clone(),
                name: spec.name,
                description: spec.description,
                domain: spec.domain,
                owner_service: spec.owner_service,
                data_type: spec.data_type,
                datasource_id: datasource_id.clone(),
                etl_pipeline_id: etl_pipeline_id.clone(),
                result_table_id: result_table_id.clone(),
                result_table_name: spec.result_table.result_table_name.clone(),
                status: spec.status,
                status_message: spec.status_message,
                created_at: now,
                updated_at: now,
            },
            datasource: DataSource {
                datasource_id,
                data_link_id: data_link_id.clone(),
                producer: spec.datasource.producer,
                data_type: spec.datasource.data_type,
                collect_method: spec.datasource.collect_method,
                protocol: spec.datasource.protocol,
                interval_seconds: spec.datasource.interval_seconds,
                labels: spec.datasource.labels,
                dimension_keys: spec.datasource.dimension_keys,
                auth_ref: spec.datasource.auth_ref,
                config: spec.datasource.config,
                created_at: now,
                updated_at: now,
            },
            etl_pipeline: EtlPipeline {
                etl_pipeline_id,
                data_link_id: data_link_id.clone(),
                mode: spec.etl_pipeline.mode,
                config: spec.etl_pipeline.config,
                created_at: now,
                updated_at: now,
            },
            result_table: ResultTable {
                result_table_id,
                data_link_id,
                result_table_name: spec.result_table.result_table_name,
                storage_type: spec.result_table.storage_type,
                storage_cluster: spec.result_table.storage_cluster,
                database: spec.result_table.database,
                table_name: spec.result_table.table_name,
                metric_name: spec.result_table.metric_name,
                query_template: spec.result_table.query_template,
                schema: spec.result_table.schema,
                retention_days: spec.result_table.retention_days,
                created_at: now,
                updated_at: now,
            },
        }
    }

    fn update_existing_bundle(
        existing: &DataLinkBundle,
        spec: ApplyDataLinkSpec,
    ) -> DataLinkBundle {
        let now = Utc::now();

        DataLinkBundle {
            data_link: DataLink {
                data_link_id: existing.data_link.data_link_id.clone(),
                name: spec.name,
                description: spec.description,
                domain: spec.domain,
                owner_service: spec.owner_service,
                data_type: spec.data_type,
                datasource_id: existing.datasource.datasource_id.clone(),
                etl_pipeline_id: existing.etl_pipeline.etl_pipeline_id.clone(),
                result_table_id: existing.result_table.result_table_id.clone(),
                result_table_name: spec.result_table.result_table_name.clone(),
                status: spec.status,
                status_message: spec.status_message,
                created_at: existing.data_link.created_at,
                updated_at: now,
            },
            datasource: DataSource {
                datasource_id: existing.datasource.datasource_id.clone(),
                data_link_id: existing.data_link.data_link_id.clone(),
                producer: spec.datasource.producer,
                data_type: spec.datasource.data_type,
                collect_method: spec.datasource.collect_method,
                protocol: spec.datasource.protocol,
                interval_seconds: spec.datasource.interval_seconds,
                labels: spec.datasource.labels,
                dimension_keys: spec.datasource.dimension_keys,
                auth_ref: spec.datasource.auth_ref,
                config: spec.datasource.config,
                created_at: existing.datasource.created_at,
                updated_at: now,
            },
            etl_pipeline: EtlPipeline {
                etl_pipeline_id: existing.etl_pipeline.etl_pipeline_id.clone(),
                data_link_id: existing.data_link.data_link_id.clone(),
                mode: spec.etl_pipeline.mode,
                config: spec.etl_pipeline.config,
                created_at: existing.etl_pipeline.created_at,
                updated_at: now,
            },
            result_table: ResultTable {
                result_table_id: existing.result_table.result_table_id.clone(),
                data_link_id: existing.data_link.data_link_id.clone(),
                result_table_name: spec.result_table.result_table_name,
                storage_type: spec.result_table.storage_type,
                storage_cluster: spec.result_table.storage_cluster,
                database: spec.result_table.database,
                table_name: spec.result_table.table_name,
                metric_name: spec.result_table.metric_name,
                query_template: spec.result_table.query_template,
                schema: spec.result_table.schema,
                retention_days: spec.result_table.retention_days,
                created_at: existing.result_table.created_at,
                updated_at: now,
            },
        }
    }

    fn remove_identity_aliases(state: &mut MemoryState, bundle: &DataLinkBundle) {
        let logical_key = format!(
            "logical:{}|{}|{}",
            bundle.data_link.domain, bundle.data_link.owner_service, bundle.data_link.name
        );

        state.identity_index.remove(&logical_key);

        state.identity_index.retain(|key, value| {
            !(key.starts_with("idempotency:") && value == &bundle.data_link.data_link_id)
        });
    }
}

impl DataLinkRepository for MemoryDataLinkRepository {
    fn apply(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        let identity_key = Self::identity_key(&options, &spec);
        let fallback_key = Self::fallback_identity_key(&spec);

        let existing_id = state
            .identity_index
            .get(&identity_key)
            .cloned()
            .or_else(|| state.identity_index.get(&fallback_key).cloned());

        let target_result_table_name = spec.result_table.result_table_name.clone();
        if let Some(indexed_id) = state.result_table_index.get(&target_result_table_name)
            && existing_id.as_deref() != Some(indexed_id.as_str())
        {
            return Err(DataLinkError::ResultTableNameConflict(format!(
                "result_table_name already exists: {target_result_table_name}"
            )));
        }

        let bundle = if let Some(existing_id) = existing_id {
            let existing = state.bundles.get(&existing_id).cloned().ok_or_else(|| {
                DataLinkError::Repository(format!("missing bundle for id: {existing_id}"))
            })?;

            Self::remove_identity_aliases(&mut state, &existing);

            let previous_table_name = existing.result_table.result_table_name.clone();
            let updated = Self::update_existing_bundle(&existing, spec);

            if previous_table_name != updated.result_table.result_table_name {
                state.result_table_index.remove(&previous_table_name);
            }

            state.result_table_index.insert(
                updated.result_table.result_table_name.clone(),
                existing_id.clone(),
            );
            state.bundles.insert(existing_id.clone(), updated.clone());
            Self::bump_order(&mut state, &existing_id);
            updated
        } else {
            let created = Self::create_bundle(spec);
            let created_id = created.data_link.data_link_id.clone();

            state.result_table_index.insert(
                created.result_table.result_table_name.clone(),
                created_id.clone(),
            );
            state.bundles.insert(created_id.clone(), created.clone());
            Self::bump_order(&mut state, &created_id);
            created
        };

        let data_link_id = bundle.data_link.data_link_id.clone();
        state
            .identity_index
            .insert(identity_key, data_link_id.clone());
        state.identity_index.insert(fallback_key, data_link_id);

        Ok(bundle)
    }

    fn apply_with_idempotency(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;
        let mut draft_state = state.clone();

        let normalized_options = ApplyDataLinkOptions {
            idempotency_key: options
                .idempotency_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        };

        if let Some(key) = normalized_options.idempotency_key.as_ref() {
            if let Some(previous_spec) = draft_state.idempotency_specs.get(key)
                && previous_spec != &spec
            {
                return Err(DataLinkError::IdempotencyConflict(format!(
                    "idempotency key conflict: {key}"
                )));
            }

            if let Some(previous_result) = draft_state.idempotency_results.get(key)
                && draft_state.idempotency_specs.get(key) == Some(&spec)
            {
                return Ok(previous_result.clone());
            }
        }

        let identity_key = Self::identity_key(&normalized_options, &spec);
        let fallback_key = Self::fallback_identity_key(&spec);

        let existing_id = draft_state
            .identity_index
            .get(&identity_key)
            .cloned()
            .or_else(|| draft_state.identity_index.get(&fallback_key).cloned());

        let target_result_table_name = spec.result_table.result_table_name.clone();
        if let Some(indexed_id) = draft_state
            .result_table_index
            .get(&target_result_table_name)
            && existing_id.as_deref() != Some(indexed_id.as_str())
        {
            return Err(DataLinkError::ResultTableNameConflict(format!(
                "result_table_name already exists: {target_result_table_name}"
            )));
        }

        let bundle = if let Some(existing_id) = existing_id {
            let existing = draft_state
                .bundles
                .get(&existing_id)
                .cloned()
                .ok_or_else(|| {
                    DataLinkError::Repository(format!("missing bundle for id: {existing_id}"))
                })?;

            Self::remove_identity_aliases(&mut draft_state, &existing);

            let previous_table_name = existing.result_table.result_table_name.clone();
            let updated = Self::update_existing_bundle(&existing, spec.clone());

            if previous_table_name != updated.result_table.result_table_name {
                draft_state.result_table_index.remove(&previous_table_name);
            }

            draft_state.result_table_index.insert(
                updated.result_table.result_table_name.clone(),
                existing_id.clone(),
            );
            draft_state
                .bundles
                .insert(existing_id.clone(), updated.clone());
            Self::bump_order(&mut draft_state, &existing_id);
            updated
        } else {
            let created = Self::create_bundle(spec.clone());
            let created_id = created.data_link.data_link_id.clone();

            draft_state.result_table_index.insert(
                created.result_table.result_table_name.clone(),
                created_id.clone(),
            );
            draft_state
                .bundles
                .insert(created_id.clone(), created.clone());
            Self::bump_order(&mut draft_state, &created_id);
            created
        };

        let data_link_id = bundle.data_link.data_link_id.clone();
        draft_state
            .identity_index
            .insert(identity_key, data_link_id.clone());
        draft_state
            .identity_index
            .insert(fallback_key, data_link_id);

        if let Some(key) = normalized_options.idempotency_key {
            draft_state.idempotency_specs.insert(key.clone(), spec);
            draft_state.idempotency_results.insert(key, bundle.clone());
        }

        *state = draft_state;

        Ok(bundle)
    }

    fn get_by_id(&self, data_link_id: &str) -> Result<Option<DataLinkBundle>> {
        let state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        Ok(state.bundles.get(data_link_id).cloned())
    }

    fn get_by_result_table_name(&self, result_table_name: &str) -> Result<Option<DataLinkBundle>> {
        let state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        let data_link_id = match state.result_table_index.get(result_table_name) {
            Some(id) => id,
            None => return Ok(None),
        };

        Ok(state.bundles.get(data_link_id).cloned())
    }

    fn list(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>> {
        let state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        let mut items: Vec<DataLinkBundle> = state
            .bundles
            .values()
            .filter(|bundle| {
                if let Some(domain) = filter.domain.as_ref()
                    && &bundle.data_link.domain != domain
                {
                    return false;
                }

                if let Some(owner_service) = filter.owner_service.as_ref()
                    && &bundle.data_link.owner_service != owner_service
                {
                    return false;
                }

                if let Some(data_type) = filter.data_type.as_ref()
                    && &bundle.data_link.data_type != data_type
                {
                    return false;
                }

                if let Some(status) = filter.status.as_ref()
                    && &bundle.data_link.status != status
                {
                    return false;
                }

                if let Some(storage_type) = filter.storage_type.as_ref()
                    && &bundle.result_table.storage_type != storage_type
                {
                    return false;
                }

                true
            })
            .cloned()
            .collect();

        items.sort_by(|a, b| {
            let order_a = state
                .order_index
                .get(&a.data_link.data_link_id)
                .copied()
                .unwrap_or(0);
            let order_b = state
                .order_index
                .get(&b.data_link.data_link_id)
                .copied()
                .unwrap_or(0);

            order_b
                .cmp(&order_a)
                .then_with(|| a.data_link.data_link_id.cmp(&b.data_link.data_link_id))
        });

        let total = items.len() as u64;
        let offset = pagination.offset() as usize;
        let limit = pagination.limit() as usize;
        let paged = items.into_iter().skip(offset).take(limit).collect();

        Ok(PaginatedResult::new(paged, total, &pagination))
    }

    fn set_status(&self, data_link_id: &str, request: SetDataLinkStatus) -> Result<DataLinkBundle> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| DataLinkError::Repository(err.to_string()))?;

        let current = state.bundles.get(data_link_id).cloned().ok_or_else(|| {
            DataLinkError::NotFound(format!("datalink not found: {data_link_id}"))
        })?;

        let now = Utc::now();
        let status_message = request.status_message.or(request.reason);
        let mut updated = current;
        updated.data_link.status = request.status;
        updated.data_link.status_message = status_message;
        updated.data_link.updated_at = now;
        updated.datasource.updated_at = now;
        updated.etl_pipeline.updated_at = now;
        updated.result_table.updated_at = now;

        state
            .bundles
            .insert(data_link_id.to_string(), updated.clone());
        Self::bump_order(&mut state, data_link_id);

        Ok(updated)
    }
}
