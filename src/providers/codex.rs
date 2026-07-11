use async_trait::async_trait;
use reqwest::Client;

use crate::context::Context;
use crate::error::OrchestratorResult;
use crate::providers::http_client;
use crate::providers::openai_compat;

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Provider OpenAI (Codex / GPT). API OpenAI-compatible, auth `Bearer`.
pub struct CodexProvider {
    client: Client,
}

impl CodexProvider {
    pub fn from_env() -> Self {
        let key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY set");
        let client = http_client::build_client(&[("Authorization", &format!("Bearer {key}"))])
            .expect("failed to build OpenAI client");
        CodexProvider { client }
    }
}

#[async_trait]
impl crate::providers::Provider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        model: &str,
        ctx: &Context,
    ) -> OrchestratorResult<String> {
        openai_compat::chat(
            &self.client,
            OPENAI_BASE_URL,
            system,
            prompt,
            model,
            ctx,
            "codex",
        )
        .await
    }
}
