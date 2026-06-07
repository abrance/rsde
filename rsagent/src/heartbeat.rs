use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

use crate::{
    clients::victoria_metrics::{VictoriaMetricsClient, VictoriaMetricsTransport},
    config_sync::SyncOutcome,
    registration::{AgentIdentity, AgentRuntimeState},
    runtime_coordinator::{
        TransientRetryPolicy, default_transient_retry_policy, retry_blocking_on_transient,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatDiagnosticTransition {
    Sent,
    Degraded,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatDiagnostic {
    pub transition: HeartbeatDiagnosticTransition,
    pub detail: String,
}

pub struct HeartbeatReporter<T> {
    transport: T,
    retry_policy: TransientRetryPolicy,
    last_sent_at: Option<DateTime<Utc>>,
    degraded: bool,
    last_diagnostic: Option<HeartbeatDiagnostic>,
}

impl<T> HeartbeatReporter<T>
where
    T: VictoriaMetricsTransport,
{
    pub fn new(transport: T) -> Self {
        Self::with_retry_policy(transport, default_transient_retry_policy())
    }

    pub fn with_retry_policy(transport: T, retry_policy: TransientRetryPolicy) -> Self {
        Self {
            transport,
            retry_policy,
            last_sent_at: None,
            degraded: false,
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
        let was_degraded = self.degraded;

        match retry_blocking_on_transient(&self.retry_policy, || {
            client.write(&mut self.transport, &payload)
        }) {
            Ok(()) => {
                self.last_sent_at = Some(now);
                self.degraded = false;
                self.last_diagnostic = Some(HeartbeatDiagnostic {
                    transition: if was_degraded {
                        HeartbeatDiagnosticTransition::Recovered
                    } else {
                        HeartbeatDiagnosticTransition::Sent
                    },
                    detail: if was_degraded {
                        "heartbeat write recovered after transient failure".to_string()
                    } else {
                        "heartbeat sent successfully".to_string()
                    },
                });
                Ok(HeartbeatTick::Sent { payload })
            }
            Err(error) => {
                self.degraded = true;
                self.last_diagnostic = Some(HeartbeatDiagnostic {
                    transition: HeartbeatDiagnosticTransition::Degraded,
                    detail: format!("heartbeat write degraded: {error}"),
                });
                Err(error)
            }
        }
    }

    pub async fn tick_async(
        mut self,
        now: DateTime<Utc>,
        state: AgentRuntimeState,
        agent_id: String,
        identity: AgentIdentity,
    ) -> (Self, Result<HeartbeatTick>)
    where
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(move || {
            let result = self.tick(now, &state, &agent_id, &identity);
            (self, result)
        })
        .await
        .expect("heartbeat blocking task panicked")
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn reset(&mut self) {
        self.last_sent_at = None;
        self.degraded = false;
        self.last_diagnostic = None;
    }

    pub fn recover(&mut self) {
        self.mark_recovered("heartbeat recovered".to_string());
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn last_diagnostic(&self) -> Option<&HeartbeatDiagnostic> {
        self.last_diagnostic.as_ref()
    }

    fn mark_recovered(&mut self, detail: String) {
        self.degraded = false;
        self.last_diagnostic = Some(HeartbeatDiagnostic {
            transition: HeartbeatDiagnosticTransition::Recovered,
            detail,
        });
    }
}

pub fn reconcile_after_sync<T>(
    reporter: &mut HeartbeatReporter<T>,
    outcome: &SyncOutcome,
    state: &AgentRuntimeState,
) where
    T: VictoriaMetricsTransport,
{
    if outcome.heartbeat_reset_required {
        reporter.reset();
    } else if reporter.is_degraded()
        && matches!(
            state.sync_state(),
            crate::registration::RuntimeSyncState::Accepted
        )
    {
        reporter.mark_recovered("heartbeat recovered after healthy sync".to_string());
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
