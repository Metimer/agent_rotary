use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::context::Context;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::providers::http_client;

const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Provider Anthropic (Claude). Endpoint `/v1/messages`, auth `x-api-key`.
pub struct ClaudeProvider {
    client: Client,
}

impl ClaudeProvider {
    pub fn from_env() -> Self {
        let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY set");
        let client = http_client::build_client(&[
            ("x-api-key", &key),
            ("anthropic-version", ANTHROPIC_VERSION),
        ])
        .expect("failed to build Anthropic client");
        ClaudeProvider { client }
    }
}

#[async_trait]
impl crate::providers::Provider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        model: &str,
        _ctx: &Context,
    ) -> OrchestratorResult<String> {
        let mut body = json!({
            "model": model,
            "max_tokens": DEFAULT_MAX_TOKENS,
            "messages": [{"role": "user", "content": prompt}],
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        let url = format!("{}/v1/messages", ANTHROPIC_BASE_URL);
        let resp = self.client.post(&url).json(&body).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(OrchestratorError::ProviderStatus {
                provider: "claude".to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let payload: Value = resp.json().await?;
        payload["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                OrchestratorError::Parse(format!("no text in claude response: {payload}"))
            })
    }
}
