# agent_rotary

Multi-LLM orchestrator & rotary workflow router. Core en Rust, bindings Python via PyO3, packaging Maturin.

## Architecture

- `src/providers/` — Trait `Provider` + implémentations (Claude, Kimi, Codex). Provider-agnostic : aucun rôle (planner/coder/reviewer) n'est couplé à un provider.
- `src/engine/` — Graphe d'exécution (Node, Edge, Workflow) avec boucles de feedback conditionnelles.
- `src/pyapi/` — Exposition PyO3 (`#[pyclass]` Workflow, Context).
- `src/async_bridge.rs` — Pont tokio ↔ asyncio via `pyo3-async-runtimes`.

## Commandes

```bash
# Build dev : compile la wheel et l'installe dans le venv courant
maturin develop

# Build release (produit target/wheels/*.whl)
maturin build --release

# Lint / format Rust
cargo fmt
cargo clippy --all-targets -- -D warnings

# Tests Rust
cargo test

# Tests Python (nécessite `maturin develop` au préalable)
pip install pytest pytest-asyncio
pytest -q
```

## Notes

- Clés API via variables d'environnement (`.env` chargé via dotenvy). Un provider n'est enregistré que si sa clé est présente.
- `pyo3/extension-module` est injecté par Maturin (auto pour pyo3 >= 0.27) — absent de Cargo.toml volontairement.
