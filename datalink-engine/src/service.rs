use crate::{
    DataLinkError, DataLinkRepository,
    error::Result,
    models::{
        ApplyDataLinkOptions, ApplyDataLinkSpec, DataLinkBundle, DataLinkListFilter,
        DataLinkStatus, EtlMode, PaginatedResult, PaginationParams, SetDataLinkStatus,
    },
};

#[derive(Debug, Clone)]
pub struct DataLinkService<R: DataLinkRepository> {
    repository: R,
}

impl<R: DataLinkRepository> DataLinkService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn apply_data_link(
        &self,
        mut spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle> {
        validate_etl_pipeline(&spec)?;
        normalize_apply_status(&mut spec)?;

        let existing_logical = self.find_by_logical_identity(&spec)?;

        if let Some(existing) = existing_logical.as_ref() {
            ensure_status_transition(&existing.data_link.status, &spec.status)?;
        }

        let target_result_table_name = spec.result_table.result_table_name.clone();
        if let Some(existing_by_table) = self
            .repository
            .get_by_result_table_name(&target_result_table_name)?
        {
            let same_logical_link = existing_logical
                .as_ref()
                .map(|logical| {
                    logical.data_link.data_link_id == existing_by_table.data_link.data_link_id
                })
                .unwrap_or(false);

            if !same_logical_link {
                return Err(DataLinkError::ResultTableNameConflict(format!(
                    "result_table_name already exists: {target_result_table_name}"
                )));
            }
        }

        self.repository
            .apply_with_idempotency(spec, normalize_apply_options(options))
    }

    pub fn get_data_link(&self, data_link_id: &str) -> Result<DataLinkBundle> {
        self.repository
            .get_by_id(data_link_id)?
            .ok_or_else(|| DataLinkError::NotFound(format!("datalink not found: {data_link_id}")))
    }

    pub fn get_data_link_by_result_table_name(
        &self,
        result_table_name: &str,
    ) -> Result<DataLinkBundle> {
        self.repository
            .get_by_result_table_name(result_table_name)?
            .ok_or_else(|| {
                DataLinkError::NotFound(format!(
                    "datalink not found by result_table_name: {result_table_name}"
                ))
            })
    }

    pub fn list_data_links(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>> {
        self.repository.list(filter, pagination)
    }

    pub fn set_data_link_status(
        &self,
        data_link_id: &str,
        request: SetDataLinkStatus,
    ) -> Result<DataLinkBundle> {
        let current = self.get_data_link(data_link_id)?;
        ensure_status_transition(&current.data_link.status, &request.status)?;

        let normalized_message = normalize_status_payload(
            &request.status,
            request.status_message.as_deref(),
            request.reason.as_deref(),
        )?;

        self.repository.set_status(
            data_link_id,
            SetDataLinkStatus {
                status: request.status,
                reason: None,
                status_message: normalized_message,
            },
        )
    }

    fn find_by_logical_identity(&self, spec: &ApplyDataLinkSpec) -> Result<Option<DataLinkBundle>> {
        let mut page = 1;
        let page_size = 100;

        loop {
            let paged = self.repository.list(
                DataLinkListFilter {
                    domain: Some(spec.domain.clone()),
                    owner_service: Some(spec.owner_service.clone()),
                    data_type: None,
                    status: None,
                    storage_type: None,
                },
                PaginationParams::new(page, page_size),
            )?;

            if let Some(found) = paged
                .items
                .into_iter()
                .find(|item| item.data_link.name == spec.name)
            {
                return Ok(Some(found));
            }

            if page >= paged.total_pages {
                return Ok(None);
            }

            page += 1;
        }
    }
}

fn validate_etl_pipeline(spec: &ApplyDataLinkSpec) -> Result<()> {
    match spec.etl_pipeline.mode {
        EtlMode::Passthrough => Ok(()),
        EtlMode::Vector => {
            if spec.etl_pipeline.config.is_empty() {
                return Err(DataLinkError::EtlPipelineInvalid(
                    "vector mode requires non-empty config".to_string(),
                ));
            }
            Ok(())
        }
    }
}

fn normalize_apply_options(options: ApplyDataLinkOptions) -> ApplyDataLinkOptions {
    ApplyDataLinkOptions {
        idempotency_key: options
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
}

fn normalize_apply_status(spec: &mut ApplyDataLinkSpec) -> Result<()> {
    spec.status_message =
        normalize_status_payload(&spec.status, spec.status_message.as_deref(), None)?;
    Ok(())
}

fn normalize_status_payload(
    status: &DataLinkStatus,
    status_message: Option<&str>,
    reason: Option<&str>,
) -> Result<Option<String>> {
    let normalized_status_message = normalize_text(status_message);
    let normalized_reason = normalize_text(reason);

    match status {
        DataLinkStatus::Active => Ok(None),
        DataLinkStatus::Disabled => normalized_status_message
            .or(normalized_reason)
            .map(Some)
            .ok_or(DataLinkError::StatusMessageRequired),
        DataLinkStatus::Draft | DataLinkStatus::Deleted => {
            Ok(normalized_status_message.or(normalized_reason))
        }
    }
}

fn normalize_text(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn ensure_status_transition(from: &DataLinkStatus, to: &DataLinkStatus) -> Result<()> {
    if matches!(
        (from, to),
        (DataLinkStatus::Deleted, DataLinkStatus::Active)
    ) {
        return Err(DataLinkError::StatusTransitionInvalid {
            from: "deleted".to_string(),
            to: "active".to_string(),
        });
    }

    Ok(())
}
