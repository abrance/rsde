use crate::{
    error::Result,
    models::{
        ApplyDataLinkOptions, ApplyDataLinkSpec, DataLinkBundle, DataLinkListFilter,
        PaginatedResult, PaginationParams, SetDataLinkStatus,
    },
};

pub trait DataLinkRepository: Send + Sync {
    fn apply(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle>;

    fn apply_with_idempotency(
        &self,
        spec: ApplyDataLinkSpec,
        options: ApplyDataLinkOptions,
    ) -> Result<DataLinkBundle>;

    fn get_by_id(&self, data_link_id: &str) -> Result<Option<DataLinkBundle>>;

    fn get_by_result_table_name(&self, result_table_name: &str) -> Result<Option<DataLinkBundle>>;

    fn list(
        &self,
        filter: DataLinkListFilter,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<DataLinkBundle>>;

    fn set_status(&self, data_link_id: &str, request: SetDataLinkStatus) -> Result<DataLinkBundle>;
}
