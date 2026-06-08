use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use tracing::{info, warn};

use crate::{
    clients::victoria_metrics::{VictoriaMetricsClient, VictoriaMetricsTransport},
    registration::{AgentIdentity, AgentRuntimeState},
    retry::{retry_blocking_transport_with_diagnostic, transport_max_attempts},
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
    last_sent_scope: Option<HeartbeatScope>,
    degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeartbeatScope {
    node_id: Option<String>,
    data_link_id: String,
    vm_base_url: String,
    interval_secs: u64,
}

impl<T> HeartbeatReporter<T>
where
    T: VictoriaMetricsTransport,
{
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            last_sent_at: None,
            last_sent_scope: None,
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
        let current_scope = heartbeat_scope(state);
        let config = match state.effective_config() {
            Some(config) => config,
            None => {
                return Ok(HeartbeatTick::Skipped {
                    reason: HeartbeatSkipReason::MissingConfig,
                });
            }
        };

        if self.last_sent_scope.as_ref() != current_scope.as_ref() {
            if self.last_sent_scope.is_some() || self.degraded {
                info!(
                    node_id = ?state.local_node_id(),
                    data_link_id = %config.heartbeat_config.data_link_id,
                    vm_base_url = %config.heartbeat_config.vm_base_url,
                    interval_secs = config.heartbeat_config.interval_secs,
                    degraded_before_reset = self.degraded,
                    "heartbeat scope changed; local schedule reset"
                );
            }
            self.last_sent_at = None;
            self.degraded = false;
        }

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

        let outcome = retry_blocking_transport_with_diagnostic("heartbeat_write", || {
            client.write(&mut self.transport, &payload)
        });

        match outcome.result {
            Ok(()) => {
                if self.degraded {
                    info!(
                        retries = outcome.diagnostic.retries,
                        max_attempts = transport_max_attempts(),
                        "heartbeat transport recovered; clearing degraded state"
                    );
                }
                self.last_sent_at = Some(now);
                self.last_sent_scope = current_scope;
                self.degraded = false;
                Ok(HeartbeatTick::Sent { payload })
            }
            Err(error) => {
                warn!(
                    retries = outcome.diagnostic.retries,
                    max_attempts = transport_max_attempts(),
                    error = %error,
                    "heartbeat transport failed; reporter entering degraded state"
                );
                self.degraded = true;
                Err(error)
            }
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn reset(&mut self) {
        if self.last_sent_at.is_some() || self.last_sent_scope.is_some() || self.degraded {
            info!("heartbeat reporter reset cleared local send schedule and degraded flag");
        }
        self.last_sent_at = None;
        self.last_sent_scope = None;
        self.degraded = false;
    }

    pub fn recover(&mut self) {
        if self.degraded {
            info!("heartbeat reporter manually cleared degraded flag without resetting schedule");
        }
        self.degraded = false;
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

fn heartbeat_scope(state: &AgentRuntimeState) -> Option<HeartbeatScope> {
    let config = state.effective_config()?;

    Some(HeartbeatScope {
        node_id: state.local_node_id().map(ToString::to_string),
        data_link_id: config.heartbeat_config.data_link_id.clone(),
        vm_base_url: config.heartbeat_config.vm_base_url.clone(),
        interval_secs: config.heartbeat_config.interval_secs,
    })
}

fn escape_tag_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
        .replace('=', "\\=")
}
