use async_trait::async_trait;
use reqwest::Client;

use crate::context::Context;
use crate::error::OrchestratorResult;
use crate::providers::http_client;
use crate::providers::openai_compat;

const MOONSHOT_BASE_URL: &str = "https://api.moonshot.ai/v1";

/// Provider Moonshot AI (Kimi). API OpenAI-compatible, auth `Bearer`.
pub struct KimiProvider {
    client: Client,
}

impl KimiProvider {
    pub fn from_env() -> Self {
        let key = std::env::var("MOONSHOT_API_KEY").expect("MOONSHOT_API_KEY set");
        let client = http_client::build_client(&[("Authorization", &format!("Bearer {key}"))])
            .expect("failed to build Moonshot client");
        KimiProvider { client }
    }
}

#[async_trait]
impl crate::providers::Provider for KimiProvider {
    fn name(&self) -> &str {
        "kimi"
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
            MOONSHOT_BASE_URL,
            system,
            prompt,
            model,
            ctx,
            "kimi",
        )
        .await
    }
}
