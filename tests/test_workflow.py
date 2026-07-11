"""Tests Python pour agent_rotary (sans appel réseau réel).

Ces tests valident l'API publique : Context (mapping dict), Workflow
(construction, erreurs sur provider inconnu), et l'import du module.
Les tests d'intégration avec vraies API nécessitent des clés et sont
exclus du CI par défaut.
"""

import pytest

from agent_rotary import Context, Workflow


# --- Context -----------------------------------------------------------------

def test_context_roundtrip():
    ctx = Context({"user_request": "hello", "count": 3, "ratio": 0.5})
    assert ctx.get("user_request") == "hello"
    assert ctx.get("count") == 3
    assert ctx.get_number("ratio") == 0.5
    assert ctx.get("missing") is None
    assert ctx.get("missing", "fallback") == "fallback"


def test_context_get_number_default():
    ctx = Context()
    assert ctx.get_number("score", 5.0) == 5.0


def test_context_setitem():
    ctx = Context()
    ctx["plan"] = "architecture"
    assert ctx.get("plan") == "architecture"
    assert "plan" in ctx
    assert "code" not in ctx


def test_context_missing_key_raises():
    ctx = Context()
    with pytest.raises(KeyError):
        _ = ctx["nope"]


def test_context_nested_dict_and_list():
    ctx = Context({"data": {"a": [1, 2, {"b": True}]}})
    data = ctx.get("data")
    assert data["a"][0] == 1
    assert data["a"][2]["b"] is True


# --- Workflow ----------------------------------------------------------------

def test_workflow_construct():
    wf = Workflow()
    # Aucune clé d'API dans l'environnement de test => providers vide.
    assert isinstance(wf.providers(), list)


def test_workflow_unknown_provider_raises():
    wf = Workflow()
    with pytest.raises(KeyError):
        wf.add_node("plan", provider="inexistant", model="x", prompt="p")


def test_workflow_edge_before_node_is_accepted():
    # L'ajout d'arêtes ne valide pas l'existence des cibles immédiatement
    # (la validation a lieu à l'exécution). On vérifie juste que ça ne panique pas.
    wf = Workflow()
    wf.add_edge("a", "b")  # toléré à la construction


def test_context_large_unsigned_integer_roundtrip():
    value = 2**63 + 17
    ctx = Context({"value": value})
    assert ctx.get("value") == value


def test_context_out_of_json_range_integer_raises():
    with pytest.raises(OverflowError):
        Context({"value": 2**128})


def test_workflow_rejects_invalid_max_steps():
    with pytest.raises(ValueError, match="max_steps"):
        Workflow(max_steps=0)

    wf = Workflow()
    with pytest.raises(ValueError, match="max_steps"):
        wf.set_max_steps(0)


def test_workflow_condition_must_be_callable():
    wf = Workflow()
    with pytest.raises(TypeError, match="callable"):
        wf.add_conditional_edge("review", "exit", True)


@pytest.mark.asyncio
async def test_execute_preserves_configuration_error_type():
    wf = Workflow()
    with pytest.raises(ValueError, match="no entry node"):
        await wf.execute(Context())
