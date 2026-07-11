//! agent_rotary — orchestrateur multi-LLM & routeur de workflows cycliques.
//!
//! Core en Rust (graphe d'exécution, async via tokio), bindings Python via PyO3,
//! packaging via Maturin.

pub mod async_bridge;
pub mod context;
pub mod engine;
pub mod error;
pub mod providers;
pub mod pyapi;

use pyo3::prelude::*;

/// Point d'entrée Python : expose les classes `Workflow` et `Context`.
#[pymodule]
fn agent_rotary(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Réchauffe le runtime tokio partagé utilisé par le pont async.
    async_bridge::init_runtime();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    m.add_class::<pyapi::PyWorkflow>()?;
    m.add_class::<pyapi::PyContext>()?;
    m.add("__doc__", "Multi-LLM orchestrator & rotary workflow router")?;
    Ok(())
}
