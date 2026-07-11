use std::collections::HashMap;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::context::Context;
use crate::engine::graph;
use crate::engine::{Edge, Node};
use crate::error::{OrchestratorError, OrchestratorResult};

pub const DEFAULT_MAX_STEPS: usize = 100;
pub const EXIT: &str = "exit";

/// Graphe d'exécution : nodes + arêtes (conditionnelles ou directes).
/// L'exécution suit les arêtes depuis `entry` jusqu'à `exit`, en évaluant
/// les conditions à chaque transition pour supporter les boucles de feedback.
#[derive(Clone)]
pub struct Workflow {
    nodes: HashMap<String, Node>,
    adj: HashMap<String, Vec<Edge>>,
    entry: Option<String>,
    max_steps: usize,
}

impl Workflow {
    pub fn new() -> Self {
        Workflow {
            nodes: HashMap::new(),
            adj: HashMap::new(),
            entry: None,
            max_steps: DEFAULT_MAX_STEPS,
        }
    }

    pub fn with_max_steps(max_steps: usize) -> OrchestratorResult<Self> {
        if max_steps == 0 {
            return Err(OrchestratorError::Config(
                "max_steps must be greater than zero".to_string(),
            ));
        }
        Ok(Workflow {
            max_steps,
            ..Self::new()
        })
    }

    pub fn set_max_steps(&mut self, max_steps: usize) -> OrchestratorResult<()> {
        if max_steps == 0 {
            return Err(OrchestratorError::Config(
                "max_steps must be greater than zero".to_string(),
            ));
        }
        self.max_steps = max_steps;
        Ok(())
    }

    pub fn set_entry(&mut self, id: impl Into<String>) {
        self.entry = Some(id.into());
    }

    pub fn add_node(&mut self, node: Node) -> OrchestratorResult<()> {
        if node.id.is_empty() {
            return Err(OrchestratorError::Config(
                "node id cannot be empty".to_string(),
            ));
        }
        if node.id == EXIT {
            return Err(OrchestratorError::Config(
                "'exit' is reserved and cannot be used as a node id".to_string(),
            ));
        }
        if self.nodes.contains_key(&node.id) {
            return Err(OrchestratorError::Config(format!(
                "duplicate node id: {}",
                node.id
            )));
        }
        if self.entry.is_none() {
            let id = node.id.clone();
            self.entry = Some(id);
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
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
    /// passe. Garde-fou `max_steps` contre les boucles infinies.
    pub async fn execute(&self, mut ctx: Context) -> OrchestratorResult<Context> {
        self.validate()?;

        let entry = self.entry.clone().ok_or(OrchestratorError::Config(
            "workflow has no entry node".to_string(),
        ))?;

        let mut current = entry;
        let mut iters = 0;

        while current != EXIT && iters < self.max_steps {
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

            let next = self.next_node(&current, &ctx)?;
            match next {
                Some(n) => current = n,
                None => {
                    return Err(OrchestratorError::DeadEnd(current));
                }
            }
            iters += 1;
        }

        if current != EXIT {
            warn!(iterations = iters, "max steps reached before exit");
            return Err(OrchestratorError::MaxSteps(self.max_steps));
        }

        info!(iterations = iters, "workflow completed");
        Ok(ctx)
    }

    /// Choisit la première arête sortante dont la condition passe.
    fn next_node(&self, from: &str, ctx: &Context) -> OrchestratorResult<Option<String>> {
        let Some(edges) = self.adj.get(from) else {
            return Ok(None);
        };
        for edge in edges {
            if edge.passes(ctx)? {
                return Ok(Some(edge.target().to_string()));
            }
        }
        Ok(None)
    }

    fn validate(&self) -> OrchestratorResult<()> {
        let entry = self
            .entry
            .as_deref()
            .ok_or_else(|| OrchestratorError::Config("workflow has no entry node".to_string()))?;
        if !self.nodes.contains_key(entry) {
            return Err(OrchestratorError::Config(format!(
                "entry points to unknown node: {entry}"
            )));
        }

        let unknown_sources = graph::unknown_sources(&self.nodes, &self.adj);
        if !unknown_sources.is_empty() {
            return Err(OrchestratorError::Config(format!(
                "edges start from unknown nodes: {}",
                unknown_sources.join(", ")
            )));
        }

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
        if !graph::can_reach_exit(entry, &self.adj) {
            return Err(OrchestratorError::Config(format!(
                "no path from entry '{entry}' to exit"
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::*;
    use crate::providers::Provider;

    struct StubProvider {
        output: String,
    }

    #[async_trait]
    impl Provider for StubProvider {
        fn name(&self) -> &str {
            "stub"
        }

        async fn complete(
            &self,
            _system: Option<&str>,
            _prompt: &str,
            _model: &str,
            _ctx: &Context,
        ) -> OrchestratorResult<String> {
            Ok(self.output.clone())
        }
    }

    fn node(id: &str, output: &str) -> Node {
        Node::new(
            id,
            Arc::new(StubProvider {
                output: output.to_string(),
            }),
            "test-model",
            "test prompt",
            None,
        )
    }

    #[tokio::test]
    async fn executes_workflow_and_extracts_json_fields() {
        let mut workflow = Workflow::new();
        workflow
            .add_node(node("review", r#"{"score":9,"feedback":"ok"}"#))
            .unwrap();
        workflow.add_edge("review", EXIT);

        let result = workflow.execute(Context::new()).await.unwrap();

        assert_eq!(result.get_number("review.score"), Some(9.0));
        assert_eq!(result.get_str("review.feedback").as_deref(), Some("ok"));
    }

    #[tokio::test]
    async fn validates_unknown_edge_source_before_execution() {
        let mut workflow = Workflow::new();
        workflow.add_node(node("known", "unused")).unwrap();
        workflow.add_edge("missing", EXIT);
        workflow.add_edge("known", EXIT);

        let error = workflow.execute(Context::new()).await.unwrap_err();

        assert!(
            matches!(error, OrchestratorError::Config(message) if message.contains("unknown nodes"))
        );
    }

    #[tokio::test]
    async fn enforces_configured_max_steps() {
        let mut workflow = Workflow::with_max_steps(1).unwrap();
        workflow.add_node(node("first", "one")).unwrap();
        workflow.add_node(node("second", "two")).unwrap();
        workflow.add_edge("first", "second");
        workflow.add_edge("second", EXIT);

        let error = workflow.execute(Context::new()).await.unwrap_err();

        assert!(matches!(error, OrchestratorError::MaxSteps(1)));
    }

    #[tokio::test]
    async fn propagates_condition_errors() {
        let mut workflow = Workflow::new();
        workflow.add_node(node("review", "done")).unwrap();
        workflow.add_conditional_edge(
            "review",
            EXIT,
            Arc::new(|_| Err(OrchestratorError::Config("condition failed".to_string()))),
        );

        let error = workflow.execute(Context::new()).await.unwrap_err();

        assert!(
            matches!(error, OrchestratorError::Config(message) if message == "condition failed")
        );
    }

    #[test]
    fn rejects_reserved_and_duplicate_node_ids() {
        let mut workflow = Workflow::new();

        assert!(workflow.add_node(node(EXIT, "unused")).is_err());
        workflow.add_node(node("step", "first")).unwrap();
        assert!(workflow.add_node(node("step", "second")).is_err());
    }

    #[tokio::test]
    async fn rejects_unknown_entry_and_missing_exit_path() {
        let mut unknown_entry = Workflow::new();
        unknown_entry.add_node(node("known", "unused")).unwrap();
        unknown_entry.add_edge("known", EXIT);
        unknown_entry.set_entry("missing");
        assert!(matches!(
            unknown_entry.execute(Context::new()).await,
            Err(OrchestratorError::Config(message)) if message.contains("entry points")
        ));

        let mut no_exit = Workflow::new();
        no_exit.add_node(node("loop", "unused")).unwrap();
        no_exit.add_edge("loop", "loop");
        assert!(matches!(
            no_exit.execute(Context::new()).await,
            Err(OrchestratorError::Config(message)) if message.contains("no path")
        ));
    }
}
