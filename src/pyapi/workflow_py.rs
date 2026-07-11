use std::sync::Arc;

use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;

use crate::context::Context;
use crate::engine::{ConditionFn, Node, Workflow};
use crate::error::OrchestratorError;
use crate::providers::ProviderRegistry;
use crate::pyapi::context_py::PyContext;

/// Workflow cyclique multi-LLM, exposé à Python.
///
/// Aucun rôle n'est couplé à un provider : l'utilisateur assigne librement
/// `provider="claude"|"kimi"|"codex"` à chaque node.
#[pyclass(name = "Workflow")]
pub struct PyWorkflow {
    inner: Workflow,
    registry: ProviderRegistry,
}

#[pymethods]
impl PyWorkflow {
    #[new]
    fn new() -> Self {
        PyWorkflow {
            inner: Workflow::new(),
            registry: ProviderRegistry::from_env(),
        }
    }

    /// Ajoute une node au workflow.
    /// `provider` doit correspondre à un nom enregistré ("claude", "kimi", "codex").
    #[pyo3(signature = (node_id, provider, model, prompt, system=None))]
    fn add_node(
        &mut self,
        node_id: &str,
        provider: &str,
        model: &str,
        prompt: &str,
        system: Option<String>,
    ) -> PyResult<()> {
        let p = self
            .registry
            .get(provider)
            .ok_or_else(|| OrchestratorError::ProviderNotFound(provider.to_string()))?;
        let node = Node::new(node_id, p, model, prompt.to_string(), system);
        self.inner.add_node(node);
        Ok(())
    }

    /// Marque la node d'entrée (sinon la première ajoutée est utilisée).
    fn set_entry(&mut self, node_id: &str) {
        self.inner.set_entry(node_id);
    }

    /// Arête directe `from -> to`.
    fn add_edge(&mut self, from: &str, to: &str) -> PyResult<()> {
        self.inner.add_edge(from, to);
        Ok(())
    }

    /// Arête conditionnelle : `to` n'est franchie que si `cond(ctx)` renvoie True.
    /// Permet les boucles de feedback (note < seuil => retour au coder).
    fn add_conditional_edge(&mut self, from: &str, to: &str, cond: Py<PyAny>) -> PyResult<()> {
        let condition: ConditionFn = make_condition(cond);
        self.inner.add_conditional_edge(from, to, condition);
        Ok(())
    }

    /// Liste les providers disponibles (ceux dont la clé d'API était présente).
    fn providers(&self) -> Vec<String> {
        self.registry.names()
    }

    /// Exécute le workflow de façon asynchrone. Renvoie un awaitable Python.
    fn execute<'py>(
        &self,
        py: Python<'py>,
        ctx: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Extraction sans emprunt prolongé : on clone le contexte Rust avant
        // de franchir la frontière async (la future doit être 'static + Send).
        let py_ctx: PyRef<'_, PyContext> = ctx.extract()?;
        let rust_ctx = py_ctx.inner.clone();
        drop(py_ctx);
        let inner = self.inner.clone();

        future_into_py(py, async move {
            let result = inner
                .execute(rust_ctx)
                .await
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
            Ok(PyContext::from(result))
        })
    }
}

/// Convertit un callable Python `cond(ctx) -> bool` en `ConditionFn` Rust
/// (`Arc<dyn Fn(&Context) -> bool + Send + Sync>`). Le callable est invoqué via
/// `Python::attach` à chaque évaluation (les conditions restent bon marché :
/// prédicats sur le contexte).
fn make_condition(callable: Py<PyAny>) -> ConditionFn {
    Arc::new(move |ctx: &Context| -> bool {
        Python::attach(|py| {
            let py_ctx = PyContext { inner: ctx.clone() };
            match callable.call1(py, (py_ctx,)) {
                Ok(res) => res.extract::<bool>(py).unwrap_or_else(|e| {
                    e.print(py);
                    false
                }),
                Err(e) => {
                    e.print(py);
                    false
                }
            }
        })
    })
}
