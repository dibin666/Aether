from __future__ import annotations

from src.services.provider.adapters.codex.quota import (
    apply_live_quota_snapshot,
    build_usage_limit_exhausted_metadata,
    is_usage_limit_reached_error,
    parse_usage_limit_reset_at,
)


def test_usage_limit_reached_payload_builds_account_level_exhausted_metadata() -> None:
    error_text = """
    {
      "error": {
        "type": "usage_limit_reached",
        "message": "The usage limit has been reached",
        "plan_type": "free",
        "resets_at": 1777546708,
        "resets_in_seconds": 513924
      }
    }
    """

    assert is_usage_limit_reached_error(error_text) is True
    assert parse_usage_limit_reset_at(error_text, now_ts=1777032784) == 1777546708

    patch = build_usage_limit_exhausted_metadata(
        error_text=error_text,
        current_namespace={"plan_type": "free"},
        now_ts=1777032784,
    )

    assert patch is not None
    assert patch["codex"]["quota_exhausted"] is True
    assert patch["codex"]["quota_exhausted_reason"] == "usage_limit_reached"
    assert patch["codex"]["quota_reset_at"] == 1777546708
    assert patch["codex"]["quota_reset_seconds"] == 513924


def test_usage_limit_reached_without_reset_window_does_not_mark_exhausted() -> None:
    error_text = (
        '{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached"}}'
    )

    assert (
        build_usage_limit_exhausted_metadata(
            error_text=error_text,
            current_namespace={"plan_type": "team"},
            now_ts=1777032784,
        )
        is None
    )


def test_apply_live_quota_snapshot_clears_account_level_exhausted_fields() -> None:
    namespace = {
        "quota_exhausted": True,
        "quota_exhausted_reason": "usage_limit_reached",
        "quota_exhausted_at": 1777032784,
        "quota_reset_at": 1777546708,
        "quota_reset_seconds": 513924,
        "legacy_marker": "keep-me",
    }
    snapshot = {
        "plan_type": "team",
        "primary_used_percent": 64.0,
        "secondary_used_percent": 3.0,
        "primary_reset_seconds": 267545,
        "secondary_reset_seconds": 411,
    }

    merged = apply_live_quota_snapshot(namespace, snapshot, now_ts=1777033000)

    assert "quota_exhausted" not in merged
    assert "quota_reset_at" not in merged
    assert merged["legacy_marker"] == "keep-me"
    assert merged["primary_used_percent"] == 64.0
    assert merged["updated_at"] == 1777033000
