from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any


_SUPPORTED_EFFORT_SUFFIXES: tuple[str, ...] = ("xhigh", "medium", "high")


@dataclass(slots=True)
class OpenAIReasoningResolution:
    requested_model: str
    routing_model: str
    reasoning_effort: str | None
    mutated_body: dict[str, Any]


def resolve_openai_reasoning_request(
    *,
    request_body: dict[str, Any],
    request_format: str,
    model_exists: Callable[[str], bool],
) -> OpenAIReasoningResolution:
    requested_model = str(request_body.get("model") or "unknown")
    mutated_body = dict(request_body)

    if not requested_model or requested_model == "unknown":
        return OpenAIReasoningResolution(
            requested_model=requested_model,
            routing_model=requested_model,
            reasoning_effort=None,
            mutated_body=mutated_body,
        )

    if model_exists(requested_model):
        return OpenAIReasoningResolution(
            requested_model=requested_model,
            routing_model=requested_model,
            reasoning_effort=None,
            mutated_body=mutated_body,
        )

    routing_model, reasoning_effort = _split_reasoning_suffix(requested_model)
    if not routing_model or not reasoning_effort or not model_exists(routing_model):
        return OpenAIReasoningResolution(
            requested_model=requested_model,
            routing_model=requested_model,
            reasoning_effort=None,
            mutated_body=mutated_body,
        )

    mutated_body["model"] = routing_model
    if request_format == "openai:cli":
        reasoning = mutated_body.get("reasoning")
        reasoning_obj = dict(reasoning) if isinstance(reasoning, dict) else {}
        reasoning_obj["effort"] = reasoning_effort
        mutated_body["reasoning"] = reasoning_obj
    else:
        mutated_body["reasoning_effort"] = reasoning_effort

    return OpenAIReasoningResolution(
        requested_model=requested_model,
        routing_model=routing_model,
        reasoning_effort=reasoning_effort,
        mutated_body=mutated_body,
    )


def _split_reasoning_suffix(model_name: str) -> tuple[str | None, str | None]:
    for effort in _SUPPORTED_EFFORT_SUFFIXES:
        suffix = f"-{effort}"
        if model_name.endswith(suffix):
            base_model = model_name[: -len(suffix)]
            if base_model:
                return base_model, effort
    return None, None
