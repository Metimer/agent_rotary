use reqwest::Client;
use serde_json::{json, Value};

use crate::context::Context;
use crate::error::{OrchestratorError, OrchestratorResult};

/// Appel de complétion partagé pour les providers compatibles OpenAI
/// (Kimi / Moonshot, Codex / OpenAI). Ne diffère que par le `base_url`.
pub async fn chat(
    client: &Client,
    base_url: &str,
    system: Option<&str>,
    prompt: &str,
    model: &str,
    _ctx: &Context,
    provider_name: &str,
) -> OrchestratorResult<String> {
    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(json!({"role": "system", "content": sys}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&json!({"model": model, "messages": messages}))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OrchestratorError::ProviderStatus {
            provider: provider_name.to_string(),
            status: status.as_u16(),
            body,
        });
    }

    let payload: Value = resp.json().await?;
    payload["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            OrchestratorError::Parse(format!("no content in {provider_name} response: {payload}"))
        })
}
