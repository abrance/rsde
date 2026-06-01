use std::{future::Future, pin::Pin};

use anyhow::{Context, Result, anyhow};
use nodemanage::{AgentSyncRequest, AgentSyncResponse};
use serde::Deserialize;

use crate::{config::AgentRuntimeConfig, registration::AgentIdentity};

pub trait NodeManageSyncTransport {
    type SyncFuture<'a>: Future<Output = Result<AgentSyncResponse>> + Send + 'a
    where
        Self: 'a;

    fn sync<'a>(
        &'a mut self,
        endpoint: &'a str,
        request: &'a AgentSyncRequest,
    ) -> Self::SyncFuture<'a>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeManageSyncClient {
    sync_url: String,
}

impl NodeManageSyncClient {
    pub fn new(sync_url: String) -> Self {
        Self { sync_url }
    }

    pub fn sync_url(&self) -> &str {
        &self.sync_url
    }

    pub fn build_request(
        config: &AgentRuntimeConfig,
        identity: &AgentIdentity,
        config_version: Option<String>,
    ) -> AgentSyncRequest {
        AgentSyncRequest {
            agent_id: config.agent_id.clone(),
            node_id: config.node_id.clone(),
            agent_version: identity.agent_version.clone(),
            hostname: identity.hostname.clone(),
            os_family: identity.os_family.clone(),
            os_distribution: identity.os_distribution.clone(),
            arch: identity.arch.clone(),
            capabilities: identity.capabilities.clone(),
            started_at: identity.started_at,
            config_version,
        }
    }

    pub fn parse_response(payload: &str) -> Result<AgentSyncResponse> {
        Ok(serde_json::from_str(payload)?)
    }

    pub async fn sync<T>(
        &self,
        transport: &mut T,
        request: &AgentSyncRequest,
    ) -> Result<AgentSyncResponse>
    where
        T: NodeManageSyncTransport,
    {
        transport.sync(self.sync_url(), request).await
    }
}

pub struct ReqwestNodeManageSyncTransport {
    client: reqwest::Client,
}

impl Default for ReqwestNodeManageSyncTransport {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl NodeManageSyncTransport for ReqwestNodeManageSyncTransport {
    type SyncFuture<'a>
        = Pin<Box<dyn Future<Output = Result<AgentSyncResponse>> + Send + 'a>>
    where
        Self: 'a;

    fn sync<'a>(
        &'a mut self,
        endpoint: &'a str,
        request: &'a AgentSyncRequest,
    ) -> Self::SyncFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .post(endpoint)
                .json(request)
                .send()
                .await
                .with_context(|| format!("failed to POST agent sync to {endpoint}"))?;

            decode_response(response, endpoint).await
        })
    }
}

#[derive(Debug, Deserialize)]
struct AgentSyncEnvelope {
    success: bool,
    data: Option<AgentSyncResponse>,
    error: Option<String>,
}

async fn decode_response(response: reqwest::Response, endpoint: &str) -> Result<AgentSyncResponse> {
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read agent sync response body from {endpoint}"))?;

    if !status.is_success() {
        return Err(anyhow!(
            "agent sync request to {endpoint} failed with status {status}: {body}"
        ));
    }

    let envelope: AgentSyncEnvelope = serde_json::from_str(&body)
        .with_context(|| format!("failed to decode agent sync response from {endpoint}"))?;

    if !envelope.success {
        return Err(anyhow!(
            "agent sync request to {endpoint} returned error: {}",
            envelope
                .error
                .unwrap_or_else(|| "unknown nodemanage error".to_string())
        ));
    }

    envelope
        .data
        .ok_or_else(|| anyhow!("agent sync response from {endpoint} missing data"))
}
