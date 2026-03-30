from __future__ import annotations

from src.models.database import ProviderAPIKey
from src.services.provider_keys.quota_reader import get_quota_reader


def _should_bypass_quota_skip(provider_type: str | None, key: ProviderAPIKey) -> bool:
    normalized_provider = str(provider_type or "").strip().lower()
    auth_type = str(getattr(key, "auth_type", "") or "").strip().lower()
    return normalized_provider == "codex" and auth_type == "oauth"


def is_key_quota_exhausted(
    provider_type: str | None,
    key: ProviderAPIKey,
    *,
    model_name: str,
) -> tuple[bool, str | None]:
    """Check ProviderAPIKey.upstream_metadata quota and decide whether to skip."""

    if _should_bypass_quota_skip(provider_type, key):
        return False, None

    reader = get_quota_reader(provider_type, getattr(key, "upstream_metadata", None))
    result = reader.is_exhausted(model_name)
    return result.exhausted, result.reason
