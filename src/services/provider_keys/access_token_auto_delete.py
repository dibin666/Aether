from __future__ import annotations

import json
from datetime import datetime, timezone
from typing import Any, Callable


SYSTEM_DELETE_ACTOR = "system:auto-delete-http400"


def _safe_json_dict(raw: str | None) -> dict[str, Any]:
    try:
        data = json.loads(raw or "{}")
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def is_access_token_only_oauth_key(*, provider: Any, key: Any, decrypt: Callable[[str], str]) -> bool:
    if str(getattr(provider, "provider_type", "")).strip().lower() != "codex":
        return False
    if str(getattr(key, "auth_type", "")).strip().lower() != "oauth":
        return False
    if not bool(getattr(key, "is_active", False)):
        return False
    access_token = str(decrypt(getattr(key, "api_key", "") or "") or "").strip()
    if not access_token:
        return False
    auth_config_raw = decrypt(getattr(key, "auth_config", "") or "{}")
    auth_config = _safe_json_dict(auth_config_raw)
    refresh_token = str(auth_config.get("refresh_token") or "").strip()
    return not refresh_token


def build_delete_log_payload(
    *,
    provider: Any,
    key: Any,
    oauth_email: str | None,
    trigger_status_code: int,
    endpoint_sig: str | None,
    proxy_node_id: str | None,
    proxy_node_name: str | None,
    request_id: str | None,
    error_message: str | None,
    raw_error_excerpt: str | None,
) -> dict[str, Any]:
    return {
        "deleted_key_id": str(getattr(key, "id", "") or ""),
        "provider_id": str(getattr(provider, "id", "") or ""),
        "provider_name": str(getattr(provider, "name", "") or ""),
        "key_name": str(getattr(key, "name", "") or ""),
        "oauth_email": str(oauth_email or "").strip() or None,
        "provider_type": str(getattr(provider, "provider_type", "") or ""),
        "auth_type": str(getattr(key, "auth_type", "") or ""),
        "trigger_status_code": int(trigger_status_code),
        "endpoint_sig": str(endpoint_sig or "").strip() or None,
        "proxy_node_id": str(proxy_node_id or "").strip() or None,
        "proxy_node_name": str(proxy_node_name or "").strip() or None,
        "request_id": str(request_id or "").strip() or None,
        "error_message": str(error_message or "").strip() or None,
        "raw_error_excerpt": str(raw_error_excerpt or "").strip() or None,
        "deleted_by": SYSTEM_DELETE_ACTOR,
        "deleted_at": datetime.now(timezone.utc),
    }
