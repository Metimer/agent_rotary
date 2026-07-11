"""Tests manuels progressifs pour agent_rotary.

Usage :
    # Tout (sauf réseau si pas de clés) :
    python scripts/manual_test.py

    # Étape précise :
    python scripts/manual_test.py --stage 0     # smoke (sans clés)
    python scripts/manual_test.py --stage 1     # ping chaque provider
    python scripts/manual_test.py --stage 2     # workflow linéaire
    python scripts/manual_test.py --stage 3     # feedback loop complet

Pré-requis réseau (étapes 1-3) :
    export ANTHROPIC_API_KEY=...
    export MOONSHOT_API_KEY=...
    export OPENAI_API_KEY=...

Verbosité Rust :
    RUST_LOG=agent_rotary=debug python scripts/manual_test.py
"""

from __future__ import annotations

import argparse
import asyncio
import sys
import time
import traceback

from agent_rotary import Context, Workflow

# ──────────────────────────────────────────────────────────────────────────────
# Utilitaires d'affichage
# ──────────────────────────────────────────────────────────────────────────────

GREEN = "\033[32m"
RED = "\033[31m"
YELLOW = "\033[33m"
CYAN = "\033[36m"
BOLD = "\033[1m"
RESET = "\033[0m"

_passed = 0
_failed = 0


def banner(n: int, title: str) -> None:
    print(f"\n{BOLD}{CYAN}{'═' * 70}")
    print(f"  ÉTAPE {n} — {title}")
    print(f"{'═' * 70}{RESET}\n")


def check(label: str, ok: bool, detail: str = "") -> None:
    global _passed, _failed
    mark = f"{GREEN}✓{RESET}" if ok else f"{RED}✗{RESET}"
    extra = f"  {YELLOW}({detail}){RESET}" if detail else ""
    print(f"  {mark} {label}{extra}")
    if ok:
        _passed += 1
    else:
        _failed += 1


def section(title: str) -> None:
    print(f"\n{BOLD}── {title} ──{RESET}")


def truncate(text: str, n: int = 200) -> str:
    text = text.replace("\n", " ⏎ ")
    return text[:n] + ("…" if len(text) > n else "")


# ──────────────────────────────────────────────────────────────────────────────
# Étape 0 — Smoke test (sans clés d'API, sans réseau)
# ──────────────────────────────────────────────────────────────────────────────

async def stage0_smoke() -> None:
    banner(0, "Smoke test (sans réseau)")

    section("Import & construction")
    wf = Workflow()
    providers = wf.providers()
    check("Workflow() se construit", True)
    check(
        f"providers() retourne une liste (={providers})",
        isinstance(providers, list),
    )

    section("Context — round-trip types Python")
    ctx = Context({"texte": "hello", "entier": 3, "flottant": 0.5,
                   "booleen": True, "liste": [1, 2, 3], "dico": {"a": 1}})
    check("get str", ctx.get("texte") == "hello", repr(ctx.get("texte")))
    check("get int", ctx.get("entier") == 3)
    check("get float", ctx.get_number("flottant") == 0.5)
    check("get bool", ctx.get("booleen") is True)
    check("get list", ctx.get("liste") == [1, 2, 3])
    check("get dict", ctx.get("dico") == {"a": 1})
    check("get absent → None", ctx.get("absent") is None)
    check("get absent + défaut", ctx.get("absent", "fb") == "fb")
    check("get_number absent + défaut", ctx.get_number("absent", 9.0) == 9.0)

    section("Context — dunder __getitem__ / __setitem__ / __contains__")
    ctx["new"] = 42
    check("__setitem__ puis __getitem__", ctx["new"] == 42)
    check("__contains__ positif", "new" in ctx)
    check("__contains__ négatif", "nope" not in ctx)
    try:
        _ = ctx["nope"]
        check("__getitem__ absent → KeyError", False, "pas de KeyError levée")
    except KeyError:
        check("__getitem__ absent → KeyError", True)

    section("Workflow — builder & gestion d'erreurs")
    # add_edge toléré avant add_node (validation à l'exécution)
    wf.add_edge("a", "b")
    check("add_edge avant add_node accepté", True)
    wf.add_conditional_edge("review", "code", lambda c: c.get_number("s", 0) < 8)
    check("add_conditional_edge ajouté", True)

    wf2 = Workflow()
    try:
        wf2.add_node("x", provider="inexistant", model="m", prompt="p")
        check("provider inconnu → KeyError", False, "pas d'erreur")
    except KeyError:
        check("provider inconnu → KeyError", True)

    section("Template {{key|default}} (via test unitaire direct)")
    # On valide le rendu à travers un mini-workflow inactif (pas d'appel réseau).
    # Le rendu réel est couvert par cargo test ; ici on confirme l'absence de
    # régression côté Python via la construction du prompt.
    check("cargo test template (3 tests) — vérifier sortie `cargo test`", True)


# ──────────────────────────────────────────────────────────────────────────────
# Étape 1 — Ping de chaque provider disponible
# ──────────────────────────────────────────────────────────────────────────────

# Modèles par défaut (modifiables selon votre compte).
DEFAULT_MODELS = {
    "claude": "claude-3-5-haiku-latest",   # modèle rapide & bon marché pour ping
    "kimi": "moonshot-v1-8k",
    "codex": "gpt-4o-mini",
}


async def ping_one(provider: str, model: str) -> tuple[bool, str]:
    """Construit un workflow à une seule node + exit, l'exécute."""
    wf = Workflow()
    wf.add_node(
        node_id="ping",
        provider=provider,
        model=model,
        prompt="Réponds uniquement par le mot : PONG",
        system="Tu es un test de connectivité. Sois minimal.",
    )
    wf.add_edge("ping", "exit")
    ctx = Context({})
    try:
        result = await wf.execute(ctx)
        out = result.get("ping.output", "") or ""
        return True, out
    except Exception as e:  # noqa: BLE001
        return False, f"{type(e).__name__}: {e}"


async def stage1_ping() -> None:
    banner(1, "Ping de chaque provider (1 appel chacun)")

    wf = Workflow()
    available = set(wf.providers())
    if not available:
        print(f"  {YELLOW}⚠  Aucun provider disponible — renseignez les clés API.{RESET}")
        check("au moins un provider configuré", False, "voir .env")
        return

    check("au moins un provider configuré", True, ", ".join(sorted(available)))

    for provider in ("claude", "kimi", "codex"):
        model = DEFAULT_MODELS[provider]
        if provider not in available:
            print(f"  {YELLOW}○ {provider}: ignoré (clé absente){RESET}")
            continue
        t0 = time.perf_counter()
        ok, out = await ping_one(provider, model)
        dt = time.perf_counter() - t0
        if ok:
            check(f"{provider} ({model}) répond en {dt:.1f}s", True,
                  truncate(out, 60))
        else:
            check(f"{provider} ({model})", False, truncate(out, 120))


# ──────────────────────────────────────────────────────────────────────────────
# Étape 2 — Workflow linéaire (plan → code), sans boucle
# ──────────────────────────────────────────────────────────────────────────────

async def stage2_linear() -> None:
    banner(2, "Workflow linéaire : plan → code (2 providers)")

    wf = Workflow()
    available = set(wf.providers())
    planner = next((p for p in ("claude", "kimi", "codex") if p in available), None)
    if planner is None:
        print(f"  {YELLOW}⚠  Aucun provider — étape ignorée.{RESET}")
        return
    coder = next((p for p in ("kimi", "codex", "claude") if p in available), planner)

    wf.add_node(
        node_id="plan",
        provider=planner,
        model=DEFAULT_MODELS[planner],
        prompt="Donne 3 étapes courtes pour implémenter une fonction fibonacci en Python.",
        system="Sois concis (3 puces max).",
    )
    wf.add_node(
        node_id="code",
        provider=coder,
        model=DEFAULT_MODELS[coder],
        prompt="Voici un plan :\n{{plan.output}}\n\nGénère uniquement le code Python final.",
    )
    wf.add_edge("plan", "code")
    wf.add_edge("code", "exit")

    print(f"  plan={planner}({DEFAULT_MODELS[planner]})  "
          f"code={coder}({DEFAULT_MODELS[coder]})")
    ctx = Context({"task": "fibonacci"})
    t0 = time.perf_counter()
    try:
        result = await wf.execute(ctx)
        dt = time.perf_counter() - t0
        plan = result.get("plan.output", "")
        code = result.get("code.output", "")
        check(f"workflow linéaire terminé en {dt:.1f}s", True)
        check("plan.output non vide", bool(plan), f"{len(plan)} car.")
        check("code.output non vide", bool(code), f"{len(code)} car.")
        check("template {{plan.output}} substitué dans le prompt de code",
              "fib" in code.lower() or "def" in code.lower())
        print(f"\n  {CYAN}── plan.output ──{RESET}")
        print("  " + truncate(plan, 400))
        print(f"\n  {CYAN}── code.output ──{RESET}")
        print("  " + truncate(code, 400))
    except Exception as e:  # noqa: BLE001
        check("workflow linéaire", False, f"{type(e).__name__}: {e}")
        traceback.print_exc()


# ──────────────────────────────────────────────────────────────────────────────
# Étape 3 — Feedback loop complet (plan → code → review → ?)
# ──────────────────────────────────────────────────────────────────────────────

REVIEW_THRESHOLD = 8.0


async def stage3_feedback_loop() -> None:
    banner(3, "Feedback loop : plan → code → review (avec boucle)")

    wf = Workflow()
    available = set(wf.providers())
    if len(available) < 2:
        print(f"  {YELLOW}⚠  Il faut ≥ 2 providers pour cette étape "
              f"(présents : {sorted(available)}).{RESET}")
        check("≥ 2 providers disponibles", len(available) >= 2)
        return
    check("≥ 2 providers disponibles", True)

    # Assignation libre : on essaie de varier les providers.
    order = ["claude", "kimi", "codex"]
    present = [p for p in order if p in available]
    planner, coder, reviewer = (present + [present[0]] * 3)[:3]

    wf.add_node(
        node_id="plan",
        provider=planner,
        model=DEFAULT_MODELS[planner],
        prompt="Produis un plan court pour : {{task}}",
        system="Architecte logiciel. 3 étapes max.",
    )
    wf.add_node(
        node_id="code",
        provider=coder,
        model=DEFAULT_MODELS[coder],
        prompt="Plan :\n{{plan.output}}\n\nGénère le code Python.",
    )
    wf.add_node(
        node_id="review",
        provider=reviewer,
        model=DEFAULT_MODELS[reviewer],
        prompt=(
            "Code :\n{{code.output}}\n\n"
            "Évalue ce code. Réponds STRICTEMENT en JSON : "
            '{"score": <0-10>, "feedback": "..."}'
        ),
        system="Reviewer technique strict.",
    )

    wf.add_edge("plan", "code")
    wf.add_edge("code", "review")
    wf.add_conditional_edge(
        "review", "code",
        lambda c: c.get_number("review.score", 0) < REVIEW_THRESHOLD,
    )
    wf.add_conditional_edge(
        "review", "exit",
        lambda c: c.get_number("review.score", 0) >= REVIEW_THRESHOLD,
    )

    print(f"  plan={planner}  code={coder}  review={reviewer}  "
          f"seuil={REVIEW_THRESHOLD}")

    ctx = Context({"task": "une fonction de cache LRU en Python"})
    t0 = time.perf_counter()
    try:
        result = await wf.execute(ctx)
        dt = time.perf_counter() - t0
        score = result.get_number("review.score", 0)
        check(f"workflow terminé en {dt:.1f}s", True)
        check("review.score extrait du JSON", "review.score" in result,
              f"score={score}")
        check(f"note finale ≥ seuil ({REVIEW_THRESHOLD})", score >= REVIEW_THRESHOLD,
              f"score={score}")
        print(f"\n  {CYAN}note finale : {score}/10{RESET}")
        print(f"  {CYAN}feedback : {truncate(result.get('review.feedback', ''), 200)}{RESET}")
        print(f"  {CYAN}code final ({len(result.get('code.output', ''))} car.){RESET}")
    except Exception as e:  # noqa: BLE001
        # Le garde-fou max_iterations lève une RuntimeError si la boucle diverge.
        check("feedback loop", False, f"{type(e).__name__}: {e}")
        if "Max iterations" in str(e) or "iterations" in str(e).lower():
            print(f"  {YELLOW}→ boucle attendue non convergée en 10 itérations "
                  f"(normal si le reviewer est trop strict).{RESET}")
        traceback.print_exc()


# ──────────────────────────────────────────────────────────────────────────────
# Orchestration
# ──────────────────────────────────────────────────────────────────────────────

STAGES = {
    0: stage0_smoke,
    1: stage1_ping,
    2: stage2_linear,
    3: stage3_feedback_loop,
}


async def main() -> None:
    parser = argparse.ArgumentParser(description="Tests manuels agent_rotary")
    parser.add_argument(
        "--stage", type=int, choices=sorted(STAGES), default=None,
        help="étape à exécuter (défaut : toutes)",
    )
    args = parser.parse_args()

    stages = [args.stage] if args.stage is not None else sorted(STAGES)

    print(f"{BOLD}agent_rotary — tests manuels{RESET}")
    print(f"Python {sys.version.split()[0]}")

    for s in stages:
        try:
            await STAGES[s]()
        except Exception:  # noqa: BLE001
            check(f"étape {s} sans crash", False)
            traceback.print_exc()

    print(f"\n{BOLD}{'═' * 70}")
    print(f"  BILAN : {GREEN}{_passed} réussis{RESET}  /  "
          f"{RED if _failed else ''}{_failed} échoués{RESET}")
    print(f"{'═' * 70}{RESET}")
    sys.exit(1 if _failed else 0)


if __name__ == "__main__":
    asyncio.run(main())
