use crate::{
    DataLinkError,
    error::Result,
    service::DataLinkService,
    storage::{memory::MemoryDataLinkRepository, mysql::MysqlDataLinkRepository},
};
use config::mysql::MysqlConfig;

pub fn build_memory_service() -> DataLinkService<MemoryDataLinkRepository> {
    DataLinkService::new(MemoryDataLinkRepository::new())
}

pub fn build_mysql_service(
    _config: MysqlConfig,
) -> Result<DataLinkService<MysqlDataLinkRepository>> {
    Err(DataLinkError::BackendNotSupported(
        "mysql service bootstrap is not implemented yet".to_string(),
    ))
}
