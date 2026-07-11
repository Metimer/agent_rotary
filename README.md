# agent_rotary

**Multi-LLM orchestrator & rotary workflow router.**

`agent_rotary` est un middleware qui orchestre plusieurs fournisseurs de LLM
(Claude / Kimi / Codex) et route des workflows cycliques entre eux. Le cœur est
écrit en Rust pour la performance et la sûreté mémoire ; il est exposé à Python
via des bindings [PyO3](https://pyo3.rs), packagés avec
[Maturin](https://www.maturin.rs).

Le cas d'usage canonique — **Plan → Code → Review → Feedback Loop** — où chaque
étape peut être assignée à *n'importe quel* provider, et où une note
insuffisante déclenche automatiquement une boucle de correction jusqu'à succès.

```text
  user_request
       │
       ▼
   ┌────────┐  plan    ┌────────┐  code   ┌──────────┐
   │  PLAN  │ ───────▶ │  CODE  │ ──────▶ │  REVIEW  │
   └────────┘          └────────┘         └──────────┘
                            ▲                   │
                            │   note < seuil    │
                            └───────────────────┘
                                  (feedback)
                                                note ≥ seuil → exit
```

---

## Table des matières

- [Pourquoi agent_rotary ?](#pourquoi-agent_rotary-)
- [Fonctionnalités](#fonctionnalités)
- [Architecture](#architecture)
- [Structure du projet](#structure-du-projet)
- [Installation](#installation)
- [Configuration](#configuration)
- [Démarrage rapide](#démarrage-rapide)
- [Référence de l'API Python](#référence-de-lapi-python)
- [Syntaxe des templates](#syntaxe-des-templates)
- [Providers supportés](#providers-supportés)
- [Le pont async Rust ↔ Python](#le-pont-async-rust--python)
- [Roadmap](#roadmap)
- [Développement](#développement)
- [Tests](#tests)
- [Licence](#licence)

---

## Pourquoi agent_rotary ?

Les LLM individuels sont puissants, mais chacun excelle dans un domaine
différent :

- **Claude (Anthropic)** brille en analyse, planification et revue critique.
- **Kimi (Moonshot AI)** offre un grand contexte utile pour la génération de
  code long.
- **Codex / GPT (OpenAI)** est polyvalent et largement éprouvé.

Orchestrer ces forces complémentaires dans un **graphe d'exécution** permet de
produire des résultats supérieurs à ceux d'un modèle unique. `agent_rotary`
 fournit :

1. **L'agnosticité** — aucun rôle (planner / coder / reviewer) n'est couplé à un
   provider. Vous décidez librement de l'assignation.
2. **Les boucles de feedback** — des transitions conditionnelles modélisent des
   cycles (revue → correction → revue) jusqu'à atteindre un critère de qualité.
3. **La performance** — le graphe, la concurrence et les appels réseau vivent en
   Rust (tokio), avec une API Python naturelle et `await`-able.

---

## Fonctionnalités

- ✅ **Graphe d'exécution (DAG + cycles)** — `Node`, `Edge` directes et
  conditionnelles, moteur d'ordonnancement avec garde-fou anti-boucle infinie.
- ✅ **Provider-agnostic** — trait `Provider` unifié, registry construit depuis
  l'environnement. Ajouter un provider = implémenter un trait.
- ✅ **Async natif** — pont `tokio` ↔ `asyncio` via `pyo3-async-runtimes`.
  `await wf.execute(ctx)` depuis Python, GIL relâché pendant les appels réseau.
- ✅ **Context dynamique** — sac clé/valeur typé (`serde_json::Value`) partagé
  entre les nodes, avec mapping bidirectionnel vers `dict` Python.
- ✅ **Templates `{{key}}`** — rendu de prompt avec substitution contextuelle et
  valeurs par défaut (`{{key|fallback}}`).
- ✅ **Extraction JSON** — si une node répond en JSON, ses champs sont exposés
  individuellement (`review.score`, `review.feedback`...).
- ✅ **Clients HTTP partagés** — `reqwest` avec `default_headers` (Bearer /
  x-api-key), timeout, TLS rustls (zéro dépendance système OpenSSL).

---

## Architecture

```text
┌──────────────────────────── Python (asyncio) ────────────────────────────┐
│                                                                          │
│   from agent_rotary import Workflow, Context                             │
│   wf = Workflow()                  # #[pyclass] PyWorkflow                │
│   await wf.execute(ctx)            # awaitable                            │
│        │                                                                  │
└────────┼──────────────────────────────────────────────────────────────────┘
         │  future_into_py  (pyo3-async-runtimes)
         ▼
┌──────────────────────────── Rust (tokio runtime) ────────────────────────┐
│                                                                          │
│   Workflow::execute()  ──▶  Node::run()  ──▶  Provider::complete()       │
│   (suivi d'arêtes)         (template)        (reqwest HTTP, GIL released) │
│                                                                          │
│   Context { data: HashMap<String, serde_json::Value> }                   │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Flux d'une exécution :**

1. Python appelle `wf.execute(ctx)` → `future_into_py` convertit une future
   Rust en awaitable Python, le runtime tokio la `spawn`.
2. Le moteur suit les arêtes depuis la node d'entrée : rend le prompt, appelle le
   provider, stocke `<node>.output` (et les champs JSON) dans le `Context`.
3. À chaque transition, les conditions d'arêtes sont évaluées ; la première qui
   passe détermine la node suivante (support natif des boucles).
4. À l'arrivée sur la node `exit`, le `Context` final est renvoyé à Python.

---

## Structure du projet

```text
agent_rotary/
├── Cargo.toml                 # dépendances Rust (pyo3 0.29, tokio, reqwest…)
├── pyproject.toml             # build-system Maturin + métadonnées PyPI
├── pytest.ini                 # config pytest (asyncio_mode = auto)
├── LICENSE                    # MIT
├── AGENTS.md                  # mémo commandes (lint / build / test)
├── src/
│   ├── lib.rs                 # #[pymodule] agent_rotary
│   ├── error.rs               # OrchestratorError → PyErr
│   ├── async_bridge.rs        # runtime tokio partagé
│   ├── context.rs             # Context dynamique
│   ├── engine/
│   │   ├── mod.rs
│   │   ├── template.rs        # rendu {{key|default}}
│   │   ├── node.rs            # Node { id, provider, model, prompts }
│   │   ├── edge.rs            # Edge::Direct | Edge::Conditional
│   │   ├── graph.rs           # validation de topologie
│   │   └── workflow.rs        # execute() + feedback loop
│   ├── providers/
│   │   ├── mod.rs             # TRAIT Provider + ProviderRegistry
│   │   ├── http_client.rs     # ClientBuilder partagé
│   │   ├── openai_compat.rs   # helper Kimi/Codex
│   │   ├── claude.rs          # Anthropic /v1/messages
│   │   ├── kimi.rs            # Moonshot /v1/chat/completions
│   │   └── codex.rs           # OpenAI /v1/chat/completions
│   └── pyapi/
│       ├── mod.rs
│       ├── context_py.rs      # #[pyclass] Context
│       └── workflow_py.rs     # #[pyclass] Workflow + future_into_py
├── examples/
│   └── main.py                # démo Plan→Code→Review→Feedback
└── tests/
    └── test_workflow.py       # tests API Python (sans réseau)
```

---

## Installation

### Depuis PyPI

```bash
pip install agent-rotary
```

### Depuis les sources (développement)

```bash
git clone https://github.com/Metimer/agent_rotary.git
cd agent_rotary

python -m venv .venv && source .venv/bin/activate
pip install maturin pytest pytest-asyncio

maturin develop          # compile la wheel et l'installe dans le venv
```

> `maturin develop` recompile et réinstalle à chaque modification du code Rust.
> L'ABI stable `abi3-py38` garantit la compatibilité Python ≥ 3.8.

---

## Configuration

Les providers sont **auto-enregistrés** depuis les variables d'environnement :
un provider absent de l'environnement n'est tout simplement pas disponible.

```bash
cp .env.example .env
# éditez .env :
#   ANTHROPIC_API_KEY=sk-ant-...
#   MOONSHOT_API_KEY=sk-...
#   OPENAI_API_KEY=sk-...
```

| Provider | Variable d'environnement | Schéma d'auth | Endpoint |
|---|---|---|---|
| Claude | `ANTHROPIC_API_KEY` | `x-api-key` | `api.anthropic.com/v1/messages` |
| Kimi | `MOONSHOT_API_KEY` | `Bearer` | `api.moonshot.ai/v1/chat/completions` |
| Codex | `OPENAI_API_KEY` | `Bearer` | `api.openai.com/v1/chat/completions` |

`.env` est chargé automatiquement via `dotenvy` au chargement du module.

---

## Démarrage rapide

```python
import asyncio
from agent_rotary import Workflow, Context

async def main():
    wf = Workflow()
    print("Providers disponibles :", wf.providers())

    # 1. PLAN — Claude génère un plan d'architecture.
    wf.add_node(
        node_id="plan",
        provider="claude",
        model="claude-sonnet-4-5",
        prompt="Produis un plan d'architecture pour : {{user_request}}",
        system="Tu es un architecte logiciel expert.",
    )

    # 2. CODE — Kimi implémente le plan.
    wf.add_node(
        node_id="code",
        provider="kimi",
        model="moonshot-v1-128k",
        prompt="Implémente ce plan :\n{{plan.output}}",
    )

    # 3. REVIEW — Codex note le code.
    wf.add_node(
        node_id="review",
        provider="codex",
        model="gpt-4o",
        prompt=(
            "Plan :\n{{plan.output}}\n\nCode :\n{{code.output}}\n\n"
            "Réponds en JSON : {\"score\": <note/10>, \"feedback\": \"...\"}."
        ),
    )

    wf.add_edge("plan", "code")
    wf.add_edge("code", "review")

    # 4. FEEDBACK LOOP : note < 8 → on renvoie à Kimi, sinon on sort.
    wf.add_conditional_edge("review", "code",
                            lambda c: c.get_number("review.score", 0) < 8)
    wf.add_conditional_edge("review", "exit",
                            lambda c: c.get_number("review.score", 0) >= 8)

    ctx = Context({"user_request": "Service de recommandation Rust + Redis."})
    result = await wf.execute(ctx)

    print("Note finale :", result.get_number("review.score", 0), "/ 10")
    print(result.get("code.output"))

asyncio.run(main())
```

Lancez-le :

```bash
python examples/main.py
```

---

## Référence de l'API Python

### `Workflow()`

Construit un workflow vide. Le registry de providers est initialisé depuis
l'environnement.

| Méthode | Description |
|---|---|
| `add_node(node_id, provider, model, prompt, system=None)` | Ajoute une étape. `provider` doit être un nom enregistré. |
| `set_entry(node_id)` | Définit la node d'entrée (sinon la première ajoutée). |
| `add_edge(from, to)` | Arête directe `from → to`. |
| `add_conditional_edge(from, to, cond)` | Arête franchie si `cond(ctx)` renvoie `True`. |
| `providers()` | Liste les noms de providers disponibles. |
| `await execute(ctx)` | Exécute le workflow. Renvoie le `Context` final. |

> Le mot réservé `"exit"` désigne la node de terminaison (n'est pas une vraie
> node). Toute arête pointant vers `"exit"` termine le workflow.

### `Context(data=None)`

Sac clé/valeur dynamique, convertible depuis/vers un `dict` Python.

| Méthode / dunder | Description |
|---|---|
| `get(key, default=None)` | Valeur convertie, ou `default`/`None`. |
| `get_number(key, default=0.0)` | Accès numérique (`score`, `count`...). |
| `set(key, value)` | Écrit une valeur. |
| `ctx[key]` / `ctx[key] = v` | Accès par indice (`__getitem__` / `__setitem__`). |
| `key in ctx` | Test de présence (`__contains__`). |
| `repr(ctx)` | Représentation lisible. |

**Conventions de nommage des clés :**

- `<node_id>.output` — réponse texte brute de la node.
- `<node_id>.<champ>` — tout champ extrait si la réponse est du JSON valide
  (ex. `review.score`, `review.feedback`).

---

## Syntaxe des templates

Les prompts supportent la substitution `{{...}}` depuis le `Context` :

| Syntaxe | Effet |
|---|---|
| `{{plan.output}}` | Remplacé par la valeur de la clé `plan.output`. |
| `{{user_request}}` | Idem, n'importe quelle clé du contexte. |
| `{{missing\|n/a}}` | Valeur par défaut si la clé est absente. |
| `{{unknown}}` (sans défaut) | Laisse `{{unknown}}` tel quel dans le texte. |

Les valeurs non-string (nombres, objets) sont stringifiées via `serde_json`.

---

## Providers supportés

### v1 (HTTP, stables)

| Provider | Nom (`provider=`) | API | Particularité |
|---|---|---|---|
| **Claude** | `"claude"` | Anthropic Messages | `system` séparé, `x-api-key`, `anthropic-version` |
| **Kimi** | `"kimi"` | OpenAI-compatible | Grand contexte, Moonshot AI |
| **Codex** | `"codex"` | OpenAI-compatible | GPT-4o et + |

Kimi et Codex partagent un helper commun (`openai_compat::chat`) ; seuls
diffèrent le `base_url` et le modèle. Claude a sa propre implémentation
(format Messages d'Anthropic).

### Ajouter un provider custom (côté Rust)

```rust
#[async_trait]
impl Provider for MonProvider {
    fn name(&self) -> &str { "monprovider" }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        model: &str,
        ctx: &Context,
    ) -> OrchestratorResult<String> {
        // ... votre logique HTTP / subprocess ...
    }
}
// registry.register(Arc::new(MonProvider::from_env()));
```

---

## Le pont async Rust ↔ Python

Le défi central : `tokio` (threads OS, pas de GIL) et `asyncio` (mono-thread,
GIL) ne partagent pas de thread. `agent_rotary` résout cela via
[`pyo3-async-runtimes`](https://github.com/PyO3/pyo3-async-runtimes)
(succession officielle de `pyo3-asyncio`) :

1. Un runtime `tokio` multi-thread est réchauffé au chargement du module
   (`async_bridge::init_runtime`).
2. `Workflow.execute()` appelle `future_into_py(py, async { ... })`, qui :
   - `spawn` la future Rust sur le runtime tokio,
   - renvoie un `asyncio.Future` awaitable côté Python.
3. Pendant l'exécution (I/O `reqwest`), **le GIL est relâché** — l'event loop
   Python reste réactive.
4. À la complétion, le résultat repasse la frontière et réveille la coroutine.

**Garanties :**

- La future Rust est `Send + 'static` : aucune donnée Python non-`Send` ne
  franchit la frontière (le `Context` est cloné avant l'appel).
- Les callables Python de conditions sont invoqués via `Python::attach` (ancien
  `with_gil`, renommé dans PyO3 0.29), brièvement, à chaque évaluation d'arête.

---

## Roadmap

- [x] **v1.0** — Claude / Kimi / Codex, feedback loops, async bridge.
- [ ] **v1.1** — Provider **OpenCode** (agent CLI invoqué en subprocess, car ce
      n'est pas une API REST standard).
- [ ] **v2.0** — **Streaming token-par-token** via async generators Python +
      `reqwest::Stream`.
- [ ] Configuration des providers par objet Python (en plus des variables d'env).
- [ ] Définition déclarative du workflow (`from_dict` / YAML) pour la
      reproductibilité.
- [ ] Parallélisation des nodes indépendantes (fan-out / fan-in).

---

## Développement

```bash
# Build dev : compile la wheel et l'installe dans le venv courant
maturin develop

# Build release (produit target/wheels/*.whl)
maturin build --release

# Formatage + lint Rust
cargo fmt
cargo clippy --all-targets -- -D warnings

# Logging : piloter la verbosité via RUST_LOG
RUST_LOG=agent_rotary=info python examples/main.py
```

**Stack technique :**

| Rôle | Technologie | Version |
|---|---|---|
| Bindings Python | [PyO3](https://pyo3.rs) | 0.29 (`abi3-py38`) |
| Pont async | [pyo3-async-runtimes](https://github.com/PyO3/pyo3-async-runtimes) | 0.29 (`tokio-runtime`) |
| Runtime async | [tokio](https://tokio.rs) | 1 |
| Client HTTP | [reqwest](https://github.com/seanmonstar/reqwest) | 0.12 (`rustls-tls`) |
| Sérialisation | serde / serde_json | 1 |
| Packaging | [Maturin](https://www.maturin.rs) | ≥ 1.5 |

---

## Tests

```bash
# Tests Rust (unitaires : template rendering)
cargo test

# Tests Python (API publique : Context, Workflow, sans réseau)
pytest -q
```

Les tests d'intégration avec de vraies API (nécessitant des clés valides)
peuvent être lancés via `python examples/main.py`.

---

## Licence

Distribué sous licence **MIT**. Voir [`LICENSE`](LICENSE).

Copyright © 2026 Metimer.
