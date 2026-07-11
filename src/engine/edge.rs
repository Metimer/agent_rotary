use std::sync::Arc;

use crate::context::Context;
use crate::error::OrchestratorResult;

/// Prédicat de transition évalué sur le `Context`.
pub type ConditionFn = Arc<dyn Fn(&Context) -> OrchestratorResult<bool> + Send + Sync>;

/// Transition entre nodes. Peut être inconditionnelle ou conditionnelle
/// (cœur des boucles de feedback : note < seuil => retour vers le coder).
#[derive(Clone)]
pub enum Edge {
    /// Transition toujours franchie.
    Direct { target: String },
    /// Franchie uniquement si `condition` renvoie `true`.
    Conditional {
        target: String,
        condition: ConditionFn,
    },
}

impl Edge {
    pub fn direct(target: impl Into<String>) -> Self {
        Edge::Direct {
            target: target.into(),
        }
    }

    pub fn conditional(target: impl Into<String>, condition: ConditionFn) -> Self {
        Edge::Conditional {
            target: target.into(),
            condition,
        }
    }

    pub fn target(&self) -> &str {
        match self {
            Edge::Direct { target } => target,
            Edge::Conditional { target, .. } => target,
        }
    }

    /// Évalue la condition (toujours `true` pour une arête directe).
    pub fn passes(&self, ctx: &Context) -> OrchestratorResult<bool> {
        match self {
            Edge::Direct { .. } => Ok(true),
            Edge::Conditional { condition, .. } => condition(ctx),
        }
    }
}
