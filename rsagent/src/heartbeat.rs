use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};

use crate::{
    clients::victoria_metrics::{VictoriaMetricsClient, VictoriaMetricsTransport},
    registration::{AgentIdentity, AgentRuntimeState},
};

const HEARTBEAT_MEASUREMENT: &str = "rsagent_heartbeat";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatSkipReason {
    MissingConfig,
    IntervalNotElapsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatTick {
    Sent { payload: String },
    Skipped { reason: HeartbeatSkipReason },
}

pub struct HeartbeatReporter<T> {
    transport: T,
    last_sent_at: Option<DateTime<Utc>>,
    degraded: bool,
}

impl<T> HeartbeatReporter<T>
where
    T: VictoriaMetricsTransport,
{
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            last_sent_at: None,
            degraded: false,
        }
    }

    pub fn tick(
        &mut self,
        now: DateTime<Utc>,
        state: &AgentRuntimeState,
        agent_id: &str,
        identity: &AgentIdentity,
    ) -> Result<HeartbeatTick> {
        let config = match state.effective_config() {
            Some(config) => config,
            None => {
                return Ok(HeartbeatTick::Skipped {
                    reason: HeartbeatSkipReason::MissingConfig,
                });
            }
        };

        if let Some(last_sent_at) = self.last_sent_at {
            let elapsed = now.signed_duration_since(last_sent_at).num_seconds();
            if elapsed >= 0 && (elapsed as u64) < config.heartbeat_config.interval_secs {
                return Ok(HeartbeatTick::Skipped {
                    reason: HeartbeatSkipReason::IntervalNotElapsed,
                });
            }
        }

        let payload = build_heartbeat_payload(state, agent_id, identity, now)?;
        let client = VictoriaMetricsClient::new(config.heartbeat_config.vm_base_url.clone());

        match client.write(&mut self.transport, &payload) {
            Ok(()) => {
                self.last_sent_at = Some(now);
                self.degraded = false;
                Ok(HeartbeatTick::Sent { payload })
            }
            Err(error) => {
                self.degraded = true;
                Err(error)
            }
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

pub fn build_heartbeat_payload(
    state: &AgentRuntimeState,
    agent_id: &str,
    identity: &AgentIdentity,
    timestamp: DateTime<Utc>,
) -> Result<String> {
    let effective_config = state
        .effective_config()
        .ok_or_else(|| anyhow!("heartbeat config unavailable"))?;
    let node_id = state
        .local_node_id()
        .ok_or_else(|| anyhow!("node_id unavailable for heartbeat"))?;

    Ok(format!(
        "{measurement},node_id={node_id},agent_id={agent_id},agent_version={agent_version},data_link_id={data_link_id},status=alive value=1i {timestamp}",
        measurement = HEARTBEAT_MEASUREMENT,
        node_id = escape_tag_value(node_id),
        agent_id = escape_tag_value(agent_id),
        agent_version = escape_tag_value(&identity.agent_version),
        data_link_id = escape_tag_value(&effective_config.heartbeat_config.data_link_id),
        timestamp = timestamp
            .timestamp_nanos_opt()
            .ok_or_else(|| anyhow!("invalid timestamp"))?,
    ))
}

fn escape_tag_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
        .replace('=', "\\=")
}
