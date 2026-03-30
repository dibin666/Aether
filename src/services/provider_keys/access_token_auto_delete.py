from __future__ import annotations

import inspect
import json
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Callable

from sqlalchemy import func

from src.core.crypto import crypto_service
from src.core.exceptions import NotFoundException
from src.models.database import AccessTokenDeleteLog, Provider, ProviderAPIKey
from src.services.provider_keys.key_side_effects import (
    cleanup_key_references,
    run_create_key_side_effects,
    run_delete_key_side_effects,
)
from src.utils.database_helpers import escape_like_pattern


SYSTEM_DELETE_ACTOR = "system:auto-delete-http400"
HTTP400_DELETE_THRESHOLD = 3
HTTP400_COUNTER_FIELD = "access_token_delete_http400_count"
HTTP400_LAST_AT_FIELD = "access_token_delete_last_http400_at"
RESTORE_STATUS_LEGACY = "legacy"
RESTORE_STATUS_PENDING = "pending"
RESTORE_STATUS_RESTORED = "restored"
RESTORE_STATUS_FAILED = "failed"


class RestoreConflictError(Exception):
    """删除历史无法恢复。"""


def _safe_json_dict(raw: str | None) -> dict[str, Any]:
    try:
        data = json.loads(raw or "{}")
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _safe_dict(raw: Any) -> dict[str, Any]:
    return dict(raw) if isinstance(raw, dict) else {}


def _parse_iso_datetime(value: Any) -> datetime | None:
    if not value:
        return None
    if isinstance(value, datetime):
        return value
    if isinstance(value, str):
        try:
            return datetime.fromisoformat(value)
        except ValueError:
            return None
    return None


def _copy_codex_metadata(key: Any) -> tuple[dict[str, Any], dict[str, Any]]:
    root = _safe_dict(getattr(key, "upstream_metadata", None))
    codex_meta = _safe_dict(root.get("codex"))
    return root, codex_meta


def _write_codex_metadata(
    key: Any,
    *,
    root: dict[str, Any],
    codex_meta: dict[str, Any],
) -> None:
    if codex_meta:
        root["codex"] = codex_meta
    elif "codex" in root:
        root.pop("codex")
    key.upstream_metadata = root or {}


def _get_http400_counter(key: Any) -> int:
    _, codex_meta = _copy_codex_metadata(key)
    return int(codex_meta.get(HTTP400_COUNTER_FIELD) or 0)


def _set_http400_counter(key: Any, *, count: int, observed_at: str | None) -> None:
    root, codex_meta = _copy_codex_metadata(key)
    if count > 0:
        codex_meta[HTTP400_COUNTER_FIELD] = count
        codex_meta[HTTP400_LAST_AT_FIELD] = observed_at
    else:
        codex_meta.pop(HTTP400_COUNTER_FIELD, None)
        codex_meta.pop(HTTP400_LAST_AT_FIELD, None)
    _write_codex_metadata(key, root=root, codex_meta=codex_meta)


def _clean_restore_upstream_metadata(raw: Any) -> dict[str, Any]:
    root = _safe_dict(raw)
    codex_meta = _safe_dict(root.get("codex"))
    codex_meta.pop(HTTP400_COUNTER_FIELD, None)
    codex_meta.pop(HTTP400_LAST_AT_FIELD, None)
    if codex_meta:
        root["codex"] = codex_meta
    else:
        root.pop("codex", None)
    return root


def _build_restore_snapshot(key: Any) -> dict[str, Any]:
    expires_at = _parse_iso_datetime(getattr(key, "expires_at", None))
    return {
        "api_formats": list(getattr(key, "api_formats", None) or []),
        "name": str(getattr(key, "name", "") or "") or "Restored Key",
        "note": getattr(key, "note", None),
        "rate_multipliers": getattr(key, "rate_multipliers", None),
        "internal_priority": int(getattr(key, "internal_priority", 50) or 50),
        "global_priority_by_format": getattr(key, "global_priority_by_format", None),
        "rpm_limit": getattr(key, "rpm_limit", None),
        "allowed_models": getattr(key, "allowed_models", None),
        "capabilities": getattr(key, "capabilities", None),
        "cache_ttl_minutes": int(getattr(key, "cache_ttl_minutes", 5) or 5),
        "max_probe_interval_minutes": int(
            getattr(key, "max_probe_interval_minutes", 32) or 32
        ),
        "auto_fetch_models": bool(getattr(key, "auto_fetch_models", False)),
        "locked_models": getattr(key, "locked_models", None),
        "model_include_patterns": getattr(key, "model_include_patterns", None),
        "model_exclude_patterns": getattr(key, "model_exclude_patterns", None),
        "is_active": bool(getattr(key, "is_active", True)),
        "proxy": getattr(key, "proxy", None),
        "fingerprint": getattr(key, "fingerprint", None),
        "expires_at": expires_at.isoformat() if expires_at else None,
        "upstream_metadata": _clean_restore_upstream_metadata(
            getattr(key, "upstream_metadata", None)
        ),
    }


def _has_restore_snapshot(item: Any) -> bool:
    return bool(
        getattr(item, "snapshot_api_key", None)
        and getattr(item, "snapshot_auth_config", None)
        and isinstance(getattr(item, "snapshot_payload", None), dict)
    )


def _can_restore_log(item: Any) -> bool:
    status = str(getattr(item, "restore_status", "") or "").strip().lower()
    return status in {RESTORE_STATUS_PENDING, RESTORE_STATUS_FAILED} and _has_restore_snapshot(item)


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
        "snapshot_api_key": getattr(key, "api_key", None),
        "snapshot_auth_config": getattr(key, "auth_config", None),
        "snapshot_payload": _build_restore_snapshot(key),
        "restore_status": RESTORE_STATUS_PENDING,
        "restored_key_id": None,
        "restored_at": None,
        "restore_error": None,
    }


def _get_key_for_delete(db: Any, key_id: str) -> Any | None:
    if hasattr(db, "query"):
        return db.query(ProviderAPIKey).filter(ProviderAPIKey.id == key_id).first()
    return getattr(db, "key", None)


def _get_delete_log_for_restore(db: Any, log_id: str) -> Any | None:
    if hasattr(db, "query"):
        return db.query(AccessTokenDeleteLog).filter(AccessTokenDeleteLog.id == log_id).first()
    return getattr(db, "delete_log", None)


def _get_provider_for_restore(db: Any, provider_id: str) -> Any | None:
    if hasattr(db, "query"):
        return db.query(Provider).filter(Provider.id == provider_id).first()
    return getattr(db, "provider", None)


def reset_access_token_only_key_http400_counter(*, db: Any, key_id: str) -> bool:
    key = _get_key_for_delete(db, key_id)
    if key is None or _get_http400_counter(key) <= 0:
        return False
    _set_http400_counter(key, count=0, observed_at=None)
    return True


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

    observed_at = datetime.now(timezone.utc).isoformat()
    observed_count = _get_http400_counter(key) + 1
    _set_http400_counter(key, count=observed_count, observed_at=observed_at)
    if observed_count < HTTP400_DELETE_THRESHOLD:
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


async def restore_access_token_delete_log(*, db: Any, log_id: str) -> ProviderAPIKey:
    item = _get_delete_log_for_restore(db, log_id)
    if item is None:
        raise NotFoundException(f"删除记录 {log_id} 不存在")

    status = str(getattr(item, "restore_status", "") or "").strip().lower()
    if status == RESTORE_STATUS_RESTORED:
        raise RestoreConflictError("该记录已恢复，不能重复撤销删除")
    if status == RESTORE_STATUS_LEGACY or not _has_restore_snapshot(item):
        raise RestoreConflictError("该记录生成于旧版本，无法恢复")

    provider_id = str(getattr(item, "provider_id", "") or "").strip()
    provider = _get_provider_for_restore(db, provider_id)
    if provider is None:
        setattr(item, "restore_status", RESTORE_STATUS_FAILED)
        setattr(item, "restore_error", f"Provider {provider_id} 不存在")
        if hasattr(db, "commit"):
            db.commit()
        raise RestoreConflictError(f"Provider {provider_id} 不存在")

    snapshot = _safe_dict(getattr(item, "snapshot_payload", None))
    now = datetime.now(timezone.utc)
    new_key = ProviderAPIKey(
        id=str(uuid.uuid4()),
        provider_id=provider_id,
        api_formats=list(snapshot.get("api_formats") or []),
        auth_type=str(getattr(item, "auth_type", "oauth") or "oauth"),
        api_key=str(getattr(item, "snapshot_api_key", "") or ""),
        auth_config=str(getattr(item, "snapshot_auth_config", "") or ""),
        name=str(snapshot.get("name") or getattr(item, "key_name", None) or "Restored Key"),
        note=snapshot.get("note"),
        rate_multipliers=snapshot.get("rate_multipliers"),
        internal_priority=int(snapshot.get("internal_priority") or 50),
        global_priority_by_format=snapshot.get("global_priority_by_format"),
        rpm_limit=snapshot.get("rpm_limit"),
        allowed_models=snapshot.get("allowed_models"),
        capabilities=snapshot.get("capabilities"),
        cache_ttl_minutes=int(snapshot.get("cache_ttl_minutes") or 5),
        max_probe_interval_minutes=int(snapshot.get("max_probe_interval_minutes") or 32),
        auto_fetch_models=bool(snapshot.get("auto_fetch_models", False)),
        locked_models=snapshot.get("locked_models"),
        model_include_patterns=snapshot.get("model_include_patterns"),
        model_exclude_patterns=snapshot.get("model_exclude_patterns"),
        proxy=snapshot.get("proxy"),
        fingerprint=snapshot.get("fingerprint"),
        expires_at=_parse_iso_datetime(snapshot.get("expires_at")),
        upstream_metadata=_clean_restore_upstream_metadata(snapshot.get("upstream_metadata")),
        oauth_invalid_at=None,
        oauth_invalid_reason=None,
        health_by_format={},
        circuit_breaker_by_format={},
        request_count=0,
        success_count=0,
        error_count=0,
        total_response_time_ms=0,
        is_active=True,
        created_at=now,
        updated_at=now,
    )

    try:
        db.add(new_key)
        item.restore_status = RESTORE_STATUS_RESTORED
        item.restored_key_id = new_key.id
        item.restored_at = now
        item.restore_error = None
        db.commit()
    except Exception as exc:
        if hasattr(db, "rollback"):
            db.rollback()
        item.restore_status = RESTORE_STATUS_FAILED
        item.restore_error = str(exc)
        if hasattr(db, "commit"):
            db.commit()
        raise

    await _maybe_await(run_create_key_side_effects(db=db, provider_id=provider_id, key=new_key))
    return new_key


def serialize_access_token_delete_log(item: AccessTokenDeleteLog) -> dict[str, Any]:
    return {
        "id": item.id,
        "deleted_key_id": item.deleted_key_id,
        "provider_id": item.provider_id,
        "provider_name": item.provider_name,
        "key_name": item.key_name,
        "oauth_email": item.oauth_email,
        "provider_type": item.provider_type,
        "auth_type": item.auth_type,
        "trigger_status_code": item.trigger_status_code,
        "endpoint_sig": item.endpoint_sig,
        "proxy_node_id": item.proxy_node_id,
        "proxy_node_name": item.proxy_node_name,
        "request_id": item.request_id,
        "error_message": item.error_message,
        "raw_error_excerpt": item.raw_error_excerpt,
        "deleted_by": item.deleted_by,
        "deleted_at": item.deleted_at.isoformat() if item.deleted_at else None,
        "restore_status": item.restore_status,
        "restored_key_id": item.restored_key_id,
        "restored_at": item.restored_at.isoformat() if item.restored_at else None,
        "restore_error": item.restore_error,
        "can_restore": _can_restore_log(item),
    }


def get_access_token_delete_summary(db: Any, *, days: int = 1) -> dict[str, int]:
    _ = days
    now = datetime.now(timezone.utc)
    today_start = now.replace(hour=0, minute=0, second=0, microsecond=0)
    last_24h_cutoff = now - timedelta(hours=24)
    total = int(db.query(func.count(AccessTokenDeleteLog.id)).scalar() or 0)
    today = int(
        db.query(func.count(AccessTokenDeleteLog.id))
        .filter(AccessTokenDeleteLog.deleted_at >= today_start)
        .scalar()
        or 0
    )
    last_24h = int(
        db.query(func.count(AccessTokenDeleteLog.id))
        .filter(AccessTokenDeleteLog.deleted_at >= last_24h_cutoff)
        .scalar()
        or 0
    )
    return {"total": total, "today": today, "last_24h": last_24h}


def list_access_token_delete_logs(
    db: Any,
    *,
    email: str | None,
    provider_id: str | None,
    days: int,
    limit: int,
    offset: int,
) -> dict[str, Any]:
    cutoff = datetime.now(timezone.utc) - timedelta(days=max(days, 1))
    query = db.query(AccessTokenDeleteLog).filter(AccessTokenDeleteLog.deleted_at >= cutoff)
    if email:
        escaped = escape_like_pattern(email.strip())
        query = query.filter(AccessTokenDeleteLog.oauth_email.ilike(f"%{escaped}%", escape="\\"))
    if provider_id:
        query = query.filter(AccessTokenDeleteLog.provider_id == provider_id)
    total = int(query.count())
    items = (
        query.order_by(AccessTokenDeleteLog.deleted_at.desc()).offset(offset).limit(limit).all()
    )
    return {
        "total": total,
        "items": [serialize_access_token_delete_log(item) for item in items],
    }
