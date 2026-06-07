use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};

use crate::{
    clients::victoria_metrics::{VictoriaMetricsClient, VictoriaMetricsTransport},
    registration::{AgentIdentity, AgentRuntimeState},
    runtime_coordinator::{RetryBackoffPolicy, next_temporary_upstream_retry_policy},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatHealthTransition {
    EnteredDegraded,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatDiagnostic {
    pub transition: HeartbeatHealthTransition,
    pub node_id: Option<String>,
    pub config_version: Option<String>,
    pub reason: Option<String>,
    pub retry_policy: Option<RetryBackoffPolicy>,
}

pub struct HeartbeatReporter<T> {
    transport: T,
    last_sent_at: Option<DateTime<Utc>>,
    degraded: bool,
    last_retry_policy: Option<RetryBackoffPolicy>,
    last_diagnostic: Option<HeartbeatDiagnostic>,
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
            last_retry_policy: None,
            last_diagnostic: None,
        }
    }

    pub fn tick(
        &mut self,
        now: DateTime<Utc>,
        state: &AgentRuntimeState,
        agent_id: &str,
        identity: &AgentIdentity,
    ) -> Result<HeartbeatTick> {
        let was_degraded = self.degraded;
        let config = match state.effective_config() {
            Some(config) => config,
            None => {
                self.last_diagnostic = None;
                return Ok(HeartbeatTick::Skipped {
                    reason: HeartbeatSkipReason::MissingConfig,
                });
            }
        };

        if let Some(last_sent_at) = self.last_sent_at {
            let elapsed = now.signed_duration_since(last_sent_at).num_seconds();
            if elapsed >= 0 && (elapsed as u64) < config.heartbeat_config.interval_secs {
                self.last_diagnostic = None;
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
                self.last_retry_policy = None;
                self.last_diagnostic =
                    (was_degraded && !self.degraded).then(|| HeartbeatDiagnostic {
                        transition: HeartbeatHealthTransition::Recovered,
                        node_id: state.local_node_id().map(ToString::to_string),
                        config_version: state.config_version().map(ToString::to_string),
                        reason: None,
                        retry_policy: None,
                    });
                Ok(HeartbeatTick::Sent { payload })
            }
            Err(error) => {
                self.degraded = true;
                self.last_retry_policy = Some(next_temporary_upstream_retry_policy(
                    self.last_retry_policy.as_ref(),
                ));
                self.last_diagnostic =
                    (!was_degraded && self.degraded).then(|| HeartbeatDiagnostic {
                        transition: HeartbeatHealthTransition::EnteredDegraded,
                        node_id: state.local_node_id().map(ToString::to_string),
                        config_version: state.config_version().map(ToString::to_string),
                        reason: Some(error.to_string()),
                        retry_policy: self.last_retry_policy.clone(),
                    });
                Err(error)
            }
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn reset(&mut self) {
        self.last_sent_at = None;
        self.degraded = false;
        self.last_retry_policy = None;
        self.last_diagnostic = None;
    }

    pub fn recover(&mut self) {
        self.degraded = false;
        self.last_retry_policy = None;
        self.last_diagnostic = Some(HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::Recovered,
            node_id: None,
            config_version: None,
            reason: None,
            retry_policy: None,
        });
    }

    pub fn last_retry_policy(&self) -> Option<&RetryBackoffPolicy> {
        self.last_retry_policy.as_ref()
    }

    pub fn last_diagnostic(&self) -> Option<&HeartbeatDiagnostic> {
        self.last_diagnostic.as_ref()
    }

    pub fn take_diagnostic(&mut self) -> Option<HeartbeatDiagnostic> {
        self.last_diagnostic.take()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

pub fn apply_sync_refresh<T>(
    reporter: &mut HeartbeatReporter<T>,
    state: &mut AgentRuntimeState,
    reset_reporter: bool,
) where
    T: VictoriaMetricsTransport,
{
    state.promote_staged_config_to_effective();

    if reset_reporter {
        reporter.reset();
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
