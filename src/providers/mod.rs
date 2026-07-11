pub mod claude;
pub mod codex;
pub mod http_client;
pub mod kimi;
pub mod openai_compat;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::context::Context;
use crate::error::OrchestratorResult;

/// Comportement commun à tous les providers. Aucun rôle (planner / coder /
/// reviewer) n'est couplé ici : c'est l'utilisateur qui assigne chaque provider
/// à une node librement.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Identifiant court ("claude", "kimi", "codex").
    fn name(&self) -> &str;

    /// Appel de complétion.
    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        model: &str,
        ctx: &Context,
    ) -> OrchestratorResult<String>;
}

/// Registre des providers disponibles, indexés par nom.
/// La construction se fait depuis les variables d'environnement : un provider
/// n'est enregistré que si sa clé est présente.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        ProviderRegistry {
            providers: HashMap::new(),
        }
    }

    /// Construit le registry par défaut en lisant les variables d'environnement.
    /// Seuls les providers dont la clé est présente sont enregistrés.
    pub fn from_env() -> Self {
        let mut r = ProviderRegistry::new();
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            r.register(Arc::new(claude::ClaudeProvider::from_env()));
        }
        if std::env::var("MOONSHOT_API_KEY").is_ok() {
            r.register(Arc::new(kimi::KimiProvider::from_env()));
        }
        if std::env::var("OPENAI_API_KEY").is_ok() {
            r.register(Arc::new(codex::CodexProvider::from_env()));
        }
        r
    }

    pub fn register(&mut self, p: Arc<dyn Provider>) {
        let name = p.name().to_string();
        self.providers.insert(name, p);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
