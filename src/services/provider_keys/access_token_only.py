"""Helpers for identifying access_token-only Codex OAuth keys."""

from __future__ import annotations

import json
from typing import Any

from src.core.crypto import crypto_service
from src.services.provider_keys.auth_type import normalize_auth_type


def _safe_json_dict(value: Any) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    if not isinstance(value, str):
        return {}
    text = value.strip()
    if not text:
        return {}
    try:
        payload = json.loads(text)
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def _load_auth_config(raw_auth_config: Any) -> dict[str, Any]:
    if isinstance(raw_auth_config, dict):
        return raw_auth_config

    direct = _safe_json_dict(raw_auth_config)
    if direct:
        return direct

    if not isinstance(raw_auth_config, str):
        return {}

    try:
        decrypted = crypto_service.decrypt(raw_auth_config)
    except Exception:
        return {}
    return _safe_json_dict(decrypted)


def is_access_token_only_codex_oauth_key(
    *,
    provider_type: str | None,
    key: Any,
) -> bool:
    normalized_provider = str(provider_type or "").strip().lower()
    if normalized_provider != "codex":
        return False

    if normalize_auth_type(str(getattr(key, "auth_type", "") or "")) != "oauth":
        return False

    auth_config = _load_auth_config(getattr(key, "auth_config", None))
    refresh_token = str(auth_config.get("refresh_token") or "").strip()
    return not refresh_token


__all__ = ["is_access_token_only_codex_oauth_key"]
