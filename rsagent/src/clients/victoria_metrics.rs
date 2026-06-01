use anyhow::{Context, Result};

pub trait VictoriaMetricsTransport {
    fn post_line_protocol(&mut self, endpoint: &str, payload: &str) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VictoriaMetricsClient {
    base_url: String,
}

impl VictoriaMetricsClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn write_endpoint(&self) -> String {
        format!("{}/api/v2/write", self.base_url.trim_end_matches('/'))
    }

    pub fn write<T>(&self, transport: &mut T, payload: &str) -> Result<()>
    where
        T: VictoriaMetricsTransport,
    {
        transport.post_line_protocol(&self.write_endpoint(), payload)
    }
}

pub struct ReqwestVictoriaMetricsTransport {
    client: reqwest::blocking::Client,
}

impl Default for ReqwestVictoriaMetricsTransport {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl VictoriaMetricsTransport for ReqwestVictoriaMetricsTransport {
    fn post_line_protocol(&mut self, endpoint: &str, payload: &str) -> Result<()> {
        let response = self
            .client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(payload.to_string())
            .send()
            .with_context(|| format!("failed to POST heartbeat to {endpoint}"))?;

        response
            .error_for_status()
            .context("VictoriaMetrics heartbeat write returned error status")?;

        Ok(())
    }
}
