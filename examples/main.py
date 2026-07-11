"""Démo agent_rotary : workflow Plan -> Code -> Review -> Feedback Loop.

Les rôles (planner / coder / reviewer) sont assignés librement par l'utilisateur
à n'importe quel provider disponible. Ici : plan=claude, code=kimi, review=codex.

Pré-requis :
    pip install agent-rotary   # ou: maturin develop
    export ANTHROPIC_API_KEY=... MOONSHOT_API_KEY=... OPENAI_API_KEY=...
"""

import asyncio
import json
import os

from agent_rotary import Context, Workflow


async def main() -> None:
    wf = Workflow()
    print(f"Providers disponibles : {wf.providers()}")

    # 1. PLAN — Claude analyse le prompt et génère un plan d'architecture.
    wf.add_node(
        node_id="plan",
        provider="claude",
        model="claude-sonnet-4-5",
        prompt="Analyse cette demande et produis un plan d'architecture concis : {{user_request}}",
        system="Tu es un architecte logiciel expert. Sois précis et structuré.",
    )

    # 2. CODE — Kimi implémente le plan.
    wf.add_node(
        node_id="code",
        provider="kimi",
        model="moonshot-v1-128k",
        prompt=(
            "Implémente ce plan en code de production (Rust) :\n\n"
            "{{plan.output}}\n\n"
            "Feedback de la review précédente :\n"
            "{{review.feedback|Aucun feedback, première version.}}"
        ),
    )

    # 3. REVIEW — Codex (OpenAI) note le code, en tenant compte du plan initial.
    wf.add_node(
        node_id="review",
        provider="codex",
        model="gpt-4o",
        prompt=(
            "Plan initial :\n{{plan.output}}\n\n"
            "Code à reviewer :\n{{code.output}}\n\n"
            "Donne une note sur 10 et des retours. Réponds STRICTEMENT "
            "au format JSON : {\"score\": <nombre>, \"feedback\": \"...\"}."
        ),
    )

    # Transitions directes : plan -> code -> review
    wf.add_edge("plan", "code")
    wf.add_edge("code", "review")

    # 4. FEEDBACK LOOP : si la note est insuffisante, on renvoie vers Kimi
    #    avec les retours, jusqu'à atteindre le seuil.
    def score_insuffisant(ctx: Context) -> bool:
        return ctx.get_number("review.score", 0.0) < 8.0

    def score_ok(ctx: Context) -> bool:
        return ctx.get_number("review.score", 0.0) >= 8.0

    wf.add_conditional_edge("review", "code", score_insuffisant)
    wf.add_conditional_edge("review", "exit", score_ok)

    ctx = Context({
        "user_request": "Un service de recommandation en Rust + Redis avec cache LRU.",
    })

    print("Démarrage du workflow (avec boucle de feedback)...\n")
    result = await wf.execute(ctx)

    print("\n" + "=" * 60)
    print("NOTE FINALE :", result.get_number("review.score", 0.0), "/ 10")
    print("=" * 60)
    print("\nCode final :\n")
    print(result.get("code.output", "<vide>"))


if __name__ == "__main__":
    asyncio.run(main())
