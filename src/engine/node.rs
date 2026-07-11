use std::sync::Arc;

use crate::context::Context;
use crate::engine::template;
use crate::error::OrchestratorResult;
use crate::providers::Provider;

pub type NodeId = String;

/// Une étape du workflow. Le `provider` et le `model` sont libres — aucun rôle
/// (planner / coder / reviewer) n'est imposé.
#[derive(Clone)]
pub struct Node {
    pub id: NodeId,
    pub provider: Arc<dyn Provider>,
    pub model: String,
    pub system_prompt: Option<String>,
    pub prompt_template: String,
}

impl Node {
    pub fn new(
        id: impl Into<String>,
        provider: Arc<dyn Provider>,
        model: impl Into<String>,
        prompt_template: impl Into<String>,
        system_prompt: Option<String>,
    ) -> Self {
        Node {
            id: id.into(),
            provider,
            model: model.into(),
            system_prompt,
            prompt_template: prompt_template.into(),
        }
    }

    /// Rend le prompt puis appelle le provider. Le résultat est destiné à être
    /// stocké dans le contexte sous `<id>.output`.
    pub async fn run(&self, ctx: &Context) -> OrchestratorResult<String> {
        let rendered = template::render(&self.prompt_template, ctx)?;
        self.provider
            .complete(self.system_prompt.as_deref(), &rendered, &self.model, ctx)
            .await
    }
}
