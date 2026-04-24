"""Codex usage_limit_reached quota helpers."""

from __future__ import annotations

import json
import time
from typing import Any

from src.services.model.upstream_fetcher import merge_upstream_metadata


def _parse_json(error_text: str | None) -> dict[str, Any] | None:
    if not isinstance(error_text, str) or not error_text.strip():
        return None
    try:
        payload = json.loads(error_text)
    except Exception:
        return None
    return payload if isinstance(payload, dict) else None


def _error_obj(payload: dict[str, Any] | None) -> dict[str, Any]:
    if not isinstance(payload, dict):
        return {}
    error_obj = payload.get("error")
    return error_obj if isinstance(error_obj, dict) else {}


def _parse_positive_int(raw: Any) -> int | None:
    if isinstance(raw, bool):
        return None
    if isinstance(raw, (int, float)):
        value = int(raw)
        return value if value > 0 else None
    if isinstance(raw, str):
        text = raw.strip()
        if not text:
            return None
        try:
            value = int(float(text))
        except Exception:
            return None
        return value if value > 0 else None
    return None


def is_usage_limit_reached_error(error_text: str | None) -> bool:
    error_obj = _error_obj(_parse_json(error_text))
    error_type = str(error_obj.get("type") or "").strip().lower()
    if error_type == "usage_limit_reached":
        return True

    message = str(error_obj.get("message") or error_text or "").strip().lower()
    return "usage limit has been reached" in message


def parse_usage_limit_reset_at(error_text: str | None, *, now_ts: int | None = None) -> int | None:
    now = int(now_ts or time.time())
    error_obj = _error_obj(_parse_json(error_text))

    reset_at = _parse_positive_int(error_obj.get("resets_at"))
    if reset_at is not None:
        return reset_at

    reset_seconds = _parse_positive_int(error_obj.get("resets_in_seconds"))
    if reset_seconds is not None:
        return now + reset_seconds

    return None


def build_usage_limit_exhausted_metadata(
    *,
    error_text: str | None,
    current_namespace: dict[str, Any] | None,
    now_ts: int | None = None,
) -> dict[str, Any] | None:
    if not is_usage_limit_reached_error(error_text):
        return None

    now = int(now_ts or time.time())
    reset_at = parse_usage_limit_reset_at(error_text, now_ts=now)
    if reset_at is None:
        return None

    payload = _parse_json(error_text)
    error_obj = _error_obj(payload)

    namespace = dict(current_namespace) if isinstance(current_namespace, dict) else {}
    namespace["quota_exhausted"] = True
    namespace["quota_exhausted_reason"] = "usage_limit_reached"
    namespace["quota_exhausted_at"] = now
    namespace["quota_reset_at"] = reset_at
    namespace["quota_reset_seconds"] = max(0, reset_at - now)
    namespace["updated_at"] = now

    plan_type = str(error_obj.get("plan_type") or "").strip().lower()
    if plan_type and not namespace.get("plan_type"):
        namespace["plan_type"] = plan_type

    return {"codex": namespace}


def apply_live_quota_snapshot(
    current_namespace: dict[str, Any] | None,
    snapshot: dict[str, Any],
    *,
    now_ts: int | None = None,
) -> dict[str, Any]:
    namespace = dict(current_namespace) if isinstance(current_namespace, dict) else {}
    for field in (
        "quota_exhausted",
        "quota_exhausted_reason",
        "quota_exhausted_at",
        "quota_reset_at",
        "quota_reset_seconds",
    ):
        namespace.pop(field, None)
    namespace.update(snapshot)
    namespace["updated_at"] = int(now_ts or time.time())
    return namespace


def sync_codex_usage_limit_state(
    *,
    db: Any,
    provider: Any,
    key: Any,
    error_text: str | None,
    request_id: str | None,
) -> bool:
    _ = request_id
    from src.core.provider_types import ProviderType, normalize_provider_type

    if key is None or provider is None:
        return False
    if normalize_provider_type(getattr(provider, "provider_type", None)) != ProviderType.CODEX:
        return False

    current_metadata = (
        key.upstream_metadata if isinstance(getattr(key, "upstream_metadata", None), dict) else {}
    )
    current_namespace = current_metadata.get("codex")
    updates = build_usage_limit_exhausted_metadata(
        error_text=error_text,
        current_namespace=current_namespace if isinstance(current_namespace, dict) else None,
    )
    if not updates:
        return False

    key.upstream_metadata = merge_upstream_metadata(current_metadata, updates)
    if hasattr(db, "add"):
        db.add(key)
    if hasattr(db, "commit"):
        db.commit()
    return True
