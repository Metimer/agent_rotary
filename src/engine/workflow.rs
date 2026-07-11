use std::collections::HashMap;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::context::Context;
use crate::engine::graph;
use crate::engine::{Edge, Node};
use crate::error::{OrchestratorError, OrchestratorResult};

const DEFAULT_MAX_ITERATIONS: usize = 10;
pub const EXIT: &str = "exit";

/// Graphe d'exécution : nodes + arêtes (conditionnelles ou directes).
/// L'exécution suit les arêtes depuis `entry` jusqu'à `exit`, en évaluant
/// les conditions à chaque transition pour supporter les boucles de feedback.
#[derive(Clone)]
pub struct Workflow {
    nodes: HashMap<String, Node>,
    adj: HashMap<String, Vec<Edge>>,
    entry: Option<String>,
}

impl Workflow {
    pub fn new() -> Self {
        Workflow {
            nodes: HashMap::new(),
            adj: HashMap::new(),
            entry: None,
        }
    }

    pub fn set_entry(&mut self, id: impl Into<String>) {
        self.entry = Some(id.into());
    }

    pub fn add_node(&mut self, node: Node) {
        if self.entry.is_none() {
            let id = node.id.clone();
            self.entry = Some(id);
        }
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.adj
            .entry(from.to_string())
            .or_default()
            .push(Edge::direct(to));
    }

    pub fn add_conditional_edge(
        &mut self,
        from: &str,
        to: &str,
        condition: crate::engine::ConditionFn,
    ) {
        self.adj
            .entry(from.to_string())
            .or_default()
            .push(Edge::conditional(to, condition));
    }

    /// Ordonnance l'exécution : node courante -> run -> stocke `<id>.output`
    /// dans le contexte -> choisit la première arête sortante dont la condition
    /// passe. Garde-fou `max_iterations` contre les boucles infinies.
    pub async fn execute(&self, mut ctx: Context) -> OrchestratorResult<Context> {
        self.validate()?;

        let entry = self.entry.clone().ok_or(OrchestratorError::Config(
            "workflow has no entry node".to_string(),
        ))?;

        let mut current = entry;
        let mut iters = 0;

        while current != EXIT && iters < DEFAULT_MAX_ITERATIONS {
            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| OrchestratorError::NodeNotFound(current.clone()))?;

            info!(node = %node.id, provider = %node.provider.name(), "running node");
            let output = node.run(&ctx).await?;
            debug!(node = %node.id, len = output.len(), "node produced output");

            // Tente d'extraire un JSON pour exposer ses champs (score, feedback...).
            if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                if let Some(obj) = parsed.as_object() {
                    for (k, v) in obj {
                        ctx.set(format!("{}.{}", node.id, k), v.clone());
                    }
                }
                ctx.set(format!("{}.output", node.id), Value::String(output.clone()));
            } else {
                ctx.set(format!("{}.output", node.id), Value::String(output));
            }

            let next = self.next_node(&current, &ctx);
            match next {
                Some(n) => current = n,
                None => {
                    return Err(OrchestratorError::DeadEnd(current));
                }
            }
            iters += 1;
        }

        if current != EXIT {
            warn!(iterations = iters, "max iterations reached before exit");
            return Err(OrchestratorError::MaxIterations(DEFAULT_MAX_ITERATIONS));
        }

        info!(iterations = iters, "workflow completed");
        Ok(ctx)
    }

    /// Choisit la première arête sortante dont la condition passe.
    fn next_node(&self, from: &str, ctx: &Context) -> Option<String> {
        self.adj.get(from).and_then(|edges| {
            edges.iter().find_map(|e| {
                if e.passes(ctx) {
                    Some(e.target().to_string())
                } else {
                    None
                }
            })
        })
    }

    fn validate(&self) -> OrchestratorResult<()> {
        let unknowns = graph::unknown_targets(&self.nodes, &self.adj);
        if !unknowns.is_empty() {
            let desc = unknowns
                .iter()
                .map(|(f, t)| format!("{f} -> {t}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(OrchestratorError::Config(format!(
                "edges point to unknown nodes: {desc}"
            )));
        }
        Ok(())
    }
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new()
    }
}
