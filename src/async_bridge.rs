/// Pont entre le runtime tokio (Rust) et la boucle asyncio (Python).
///
/// `pyo3-async-runtimes` gère un runtime tokio partagé. On se contente de le
/// réchauffer une fois au chargement du module ; `future_into_py` l'utilise
/// ensuite pour `spawn` les futures Rust et les exposer comme awaitables Python.
pub fn init_runtime() {
    // Accès statique : crée/récupère le runtime par défaut. Appel idempotent.
    let _ = pyo3_async_runtimes::tokio::get_runtime();
}
