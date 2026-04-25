from __future__ import annotations

import pytest

from src.api.admin.pool import routes as pool_routes
from src.api.admin.pool.routes import (
    _build_quota_counts_by_provider,
    _resolve_overview_quota_bucket,
)
from src.services.provider.pool.account_state import QuotaStatusSnapshot


def test_build_quota_counts_by_provider_groups_available_exhausted_and_unknown() -> None:
    result = _build_quota_counts_by_provider(
        [
            (
                "provider-codex",
                {
                    "codex": {
                        "primary_used_percent": 35.0,
                        "secondary_used_percent": 10.0,
                    }
                },
            ),
            (
                "provider-codex",
                {
                    "codex": {
                        "primary_used_percent": 100.0,
                        "secondary_used_percent": 25.0,
                    }
                },
            ),
            ("provider-codex", None),
            (
                "provider-kiro",
                {
                    "kiro": {
                        "usage_percentage": 45.0,
                        "next_reset_at": 2_000_000_000,
                    }
                },
            ),
        ],
        {
            "provider-codex": "codex",
            "provider-kiro": "kiro",
        },
    )

    assert result["provider-codex"] == {
        "available": 1,
        "exhausted": 1,
        "unknown": 1,
    }
    assert result["provider-kiro"] == {
        "available": 1,
        "exhausted": 0,
        "unknown": 0,
    }


def test_build_quota_counts_by_provider_counts_codex_low_quota_as_exhausted() -> None:
    result = _build_quota_counts_by_provider(
        [
            (
                "provider-codex",
                {
                    "codex": {
                        "primary_used_percent": 98.2,
                        "secondary_used_percent": 10.0,
                    }
                },
            ),
            (
                "provider-codex",
                {
                    "codex": {
                        "primary_used_percent": 20.0,
                        "secondary_used_percent": 98.0,
                    }
                },
            ),
            (
                "provider-codex",
                {
                    "codex": {
                        "primary_used_percent": 35.0,
                        "secondary_used_percent": 12.0,
                    }
                },
            ),
        ],
        {
            "provider-codex": "codex",
        },
    )

    assert result["provider-codex"] == {
        "available": 1,
        "exhausted": 2,
        "unknown": 0,
    }


def test_resolve_overview_quota_bucket_uses_shared_exhausted_logic() -> None:
    assert (
        _resolve_overview_quota_bucket(
            provider_type="codex",
            upstream_metadata={
                "codex": {
                    "primary_used_percent": 98.0,
                    "secondary_used_percent": 10.0,
                }
            },
        )
        == "exhausted"
    )
    assert (
        _resolve_overview_quota_bucket(
            provider_type="codex",
            upstream_metadata={
                "codex": {
                    "quota_exhausted": True,
                    "quota_exhausted_reason": "usage_limit_reached",
                }
            },
        )
        == "exhausted"
    )
    assert (
        _resolve_overview_quota_bucket(
            provider_type="codex",
            upstream_metadata={
                "codex": {
                    "primary_used_percent": 35.0,
                    "secondary_used_percent": 12.0,
                }
            },
        )
        == "available"
    )
    assert (
        _resolve_overview_quota_bucket(
            provider_type="codex",
            upstream_metadata=None,
        )
        == "unknown"
    )


def test_resolve_overview_quota_bucket_delegates_to_shared_quota_snapshot(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    observed: dict[str, object] = {}

    def fake_resolve_quota_status_snapshot(
        *,
        provider_type: str | None,
        upstream_metadata: object,
    ) -> QuotaStatusSnapshot:
        observed["provider_type"] = provider_type
        observed["upstream_metadata"] = upstream_metadata
        return QuotaStatusSnapshot(code="ok", exhausted=False)

    monkeypatch.setattr(
        pool_routes,
        "resolve_quota_status_snapshot",
        fake_resolve_quota_status_snapshot,
    )

    assert (
        pool_routes._resolve_overview_quota_bucket(
            provider_type="codex",
            upstream_metadata={"codex": {"primary_used_percent": 50.0}},
        )
        == "available"
    )
    assert observed == {
        "provider_type": "codex",
        "upstream_metadata": {"codex": {"primary_used_percent": 50.0}},
    }
