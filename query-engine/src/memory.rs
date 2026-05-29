use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{HeartbeatQuery, HeartbeatSample, HeartbeatStore, QueryEngineError, Result};

#[derive(Debug, Clone, Default)]
pub struct InMemoryHeartbeatStore {
    state: Arc<Mutex<HashMap<(String, String), HeartbeatSample>>>,
}

impl InMemoryHeartbeatStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, result_table_name: impl Into<String>, sample: HeartbeatSample) {
        let mut state = self.state.lock().expect("heartbeat state lock poisoned");
        state.insert((result_table_name.into(), sample.node_id.clone()), sample);
    }
}

impl HeartbeatStore for InMemoryHeartbeatStore {
    fn latest(&self, query: &HeartbeatQuery) -> Result<Option<HeartbeatSample>> {
        let state = self
            .state
            .lock()
            .map_err(|err| QueryEngineError::HeartbeatStore(err.to_string()))?;
        Ok(state
            .get(&(query.result_table_name.clone(), query.node_id.clone()))
            .cloned())
    }
}
