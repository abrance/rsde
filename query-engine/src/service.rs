use datalink_engine::DataLinkService;

use crate::{HeartbeatQuery, HeartbeatSample, Result};

pub trait HeartbeatStore: Clone + Send + Sync + 'static {
    fn latest(&self, query: &HeartbeatQuery) -> Result<Option<HeartbeatSample>>;
}

#[derive(Debug, Clone)]
pub struct QueryEngine<D, H>
where
    D: datalink_engine::DataLinkRepository,
{
    datalink_service: DataLinkService<D>,
    heartbeat_store: H,
}

impl<D, H> QueryEngine<D, H>
where
    D: datalink_engine::DataLinkRepository,
    H: HeartbeatStore,
{
    pub fn new(datalink_service: DataLinkService<D>, heartbeat_store: H) -> Self {
        Self {
            datalink_service,
            heartbeat_store,
        }
    }

    pub fn latest_heartbeat(
        &self,
        result_table_name: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Result<Option<HeartbeatSample>> {
        let result_table_name = result_table_name.into();
        self.datalink_service
            .get_data_link_by_result_table_name(&result_table_name)?;

        self.latest_heartbeat_from_resolved_table(result_table_name, node_id.into())
    }

    pub fn latest_heartbeat_by_data_link_id(
        &self,
        data_link_id: impl AsRef<str>,
        node_id: impl Into<String>,
    ) -> Result<Option<HeartbeatSample>> {
        let bundle = self.datalink_service.get_data_link(data_link_id.as_ref())?;

        self.latest_heartbeat_from_resolved_table(
            bundle.result_table.result_table_name,
            node_id.into(),
        )
    }

    fn latest_heartbeat_from_resolved_table(
        &self,
        result_table_name: String,
        node_id: String,
    ) -> Result<Option<HeartbeatSample>> {
        self.heartbeat_store.latest(&HeartbeatQuery {
            result_table_name,
            node_id,
        })
    }
}
