use crate::{
    DataLinkError, DataLinkRepository,
    error::Result,
    models::{
        ApplyDataLinkOptions, ApplyDataLinkSpec, DataLinkBundle, DataLinkListFilter,
        PaginatedResult, PaginationParams, SetDataLinkStatus,
    },
};

#[derive(Debug, Default)]
pub struct MysqlDataLinkRepository;

impl MysqlDataLinkRepository {
    pub fn new() -> Self {
        Self
    }

    fn not_supported<T>() -> Result<T> {
        Err(DataLinkError::BackendNotSupported(
            "mysql backend is not implemented yet".to_string(),
        ))
    }
}

impl DataLinkRepository for MysqlDataLinkRepository {
    fn apply(
        &self,
        _spec: ApplyDataLinkSpec,
        _options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle> {
        Self::not_supported()
    }

    fn apply_with_idempotency(
        &self,
        _spec: ApplyDataLinkSpec,
        _options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle> {
        Self::not_supported()
    }

    fn get_by_id(&self, _data_link_id: &str) -> Result<Option<DataLinkBundle>> {
        Self::not_supported()
    }

    fn get_by_result_table_name(&self, _result_table_name: &str) -> Result<Option<DataLinkBundle>> {
        Self::not_supported()
    }

    fn list(
        &self,
        _filter: DataLinkListFilter,
        _pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>> {
        Self::not_supported()
    }

    fn set_status(
        &self,
        _data_link_id: &str,
        _request: SetDataLinkStatus,
    ) -> Result<DataLinkBundle> {
        Self::not_supported()
    }
}
