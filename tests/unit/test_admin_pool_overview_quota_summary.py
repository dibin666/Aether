from __future__ import annotations

from src.api.admin.pool.routes import _build_quota_counts_by_provider


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
