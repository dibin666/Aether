from __future__ import annotations

from src.models.database import ProviderAPIKey
from src.services.provider_keys.access_token_only import is_access_token_only_codex_oauth_key
from src.services.provider_keys.quota_reader import get_quota_reader


def _should_bypass_quota_skip(provider_type: str | None, key: ProviderAPIKey) -> bool:
    return is_access_token_only_codex_oauth_key(provider_type=provider_type, key=key)


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
