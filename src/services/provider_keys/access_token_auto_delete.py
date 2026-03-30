from __future__ import annotations

import inspect
import json
from datetime import datetime, timezone
from typing import Any, Callable

from src.core.crypto import crypto_service
from src.models.database import AccessTokenDeleteLog, ProviderAPIKey
from src.services.provider_keys.key_side_effects import (
    cleanup_key_references,
    run_delete_key_side_effects,
)


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


def _get_key_for_delete(db: Any, key_id: str) -> Any | None:
    if hasattr(db, "query"):
        return db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
    return getattr(db, "key", None)


async def _maybe_await(value: Any) -> Any:
    if inspect.isawaitable(value):
        return await value
    return value


async def delete_access_token_only_key_on_http400(
    *,
    db: Any,
    provider: Any,
    key_id: str,
    status_code: int,
    endpoint_sig: str | None,
    request_id: str | None,
    error_message: str | None,
    raw_error_excerpt: str | None,
    proxy_node_id: str | None,
    proxy_node_name: str | None,
) -> bool:
    if int(status_code) != 400:
        return False

    key = _get_key_for_delete(db, key_id)
    if key is None:
        return False
    if not is_access_token_only_oauth_key(provider=provider, key=key, decrypt=crypto_service.decrypt):
        return False

    auth_config = _safe_json_dict(crypto_service.decrypt(getattr(key, "auth_config", "") or "{}"))
    payload = build_delete_log_payload(
        provider=provider,
        key=key,
        oauth_email=auth_config.get("email"),
        trigger_status_code=status_code,
        endpoint_sig=endpoint_sig,
        proxy_node_id=proxy_node_id,
        proxy_node_name=proxy_node_name,
        request_id=request_id,
        error_message=error_message,
        raw_error_excerpt=raw_error_excerpt,
    )

    delete_log = AccessTokenDeleteLog(**payload)
    deleted_key_allowed_models = getattr(key, "allowed_models", None)
    provider_id = getattr(key, "provider_id", None)

    try:
        db.add(delete_log)
        cleanup_key_references(db, [key_id])
        db.delete(key)
        db.commit()
    except Exception:
        if hasattr(db, "rollback"):
            db.rollback()
        raise

    await _maybe_await(
        run_delete_key_side_effects(
            db=db,
            provider_id=provider_id,
            deleted_key_allowed_models=deleted_key_allowed_models,
        )
    )
    return True
