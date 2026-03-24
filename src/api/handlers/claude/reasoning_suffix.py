from __future__ import annotations

import re
from collections.abc import Callable
from typing import Any

from src.core.api_format.conversion.field_mappings import REASONING_EFFORT_TO_CLAUDE_EFFORT

_REASONING_SUFFIX_RE = re.compile(r"^(?P<base>.+)-(?P<effort>xhigh|high|medium)$")


def prepare_claude_request_for_dispatch(
    request_body: dict[str, Any],
    *,
    model_exists: Callable[[str], bool],
) -> tuple[str, str, dict[str, Any]]:
    requested_model = str(request_body.get("model") or "unknown")
    normalized_model = requested_model.strip()
    if not normalized_model:
        return requested_model, requested_model, dict(request_body)

    # 精确模型名优先，避免误伤真实存在的全局模型。
    if model_exists(normalized_model):
        return normalized_model, normalized_model, dict(request_body)

    match = _REASONING_SUFFIX_RE.fullmatch(normalized_model)
    if not match:
        return normalized_model, normalized_model, dict(request_body)

    routing_model = match.group("base").strip()
    reasoning_effort = match.group("effort")
    if not routing_model or not model_exists(routing_model):
        return normalized_model, normalized_model, dict(request_body)

    dispatch_request_body = dict(request_body)
    dispatch_request_body["model"] = routing_model

    output_config = request_body.get("output_config")
    output_config_dict = dict(output_config) if isinstance(output_config, dict) else {}
    output_config_dict["effort"] = REASONING_EFFORT_TO_CLAUDE_EFFORT[reasoning_effort]
    dispatch_request_body["output_config"] = output_config_dict

    return normalized_model, routing_model, dispatch_request_body
