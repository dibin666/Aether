from __future__ import annotations

from datetime import datetime, timedelta, timezone
from types import SimpleNamespace
from unittest.mock import AsyncMock, MagicMock

import pytest

from src.services.provider_keys.pool_quota_probe_scheduler import (
    PoolQuotaProbeScheduler,
    _select_probe_key_ids,
)


def _key(
    key_id: str,
    *,
    last_used_at: datetime | None = None,
    upstream_metadata: dict | None = None,
) -> SimpleNamespace:
    return SimpleNamespace(
        id=key_id,
        last_used_at=last_used_at,
        upstream_metadata=upstream_metadata or {},
    )


def test_select_probe_key_ids_selects_silent_keys_only() -> None:
    now = datetime(2026, 3, 5, 12, 0, 0, tzinfo=timezone.utc)
    now_ts = int(now.timestamp())

    keys = [
        _key("k1"),  # never used, should be probed
        _key("k2", last_used_at=now - timedelta(minutes=2)),  # recently used,仍可定期探测
        _key(
            "k3",
            upstream_metadata={"codex": {"updated_at": now_ts - (20 * 60)}},
        ),  # long-time no refresh, should be probed
    ]

    selected = _select_probe_key_ids(
        keys=keys,  # type: ignore[arg-type]
        provider_type="codex",
        now_ts=now_ts,
        interval_seconds=10 * 60,
        last_probe_timestamps={},
        limit=0,
    )
    assert selected == ["k1", "k2", "k3"]


def test_select_probe_key_ids_keeps_periodic_probe_even_after_recent_usage() -> None:
    now = datetime(2026, 3, 5, 12, 0, 0, tzinfo=timezone.utc)
    now_ts = int(now.timestamp())

    keys = [
        _key(
            "k1",
            last_used_at=now - timedelta(seconds=30),
            upstream_metadata={"codex": {"updated_at": now_ts - (40 * 60)}},
        )
    ]

    # 即使 key 刚刚被真实流量使用，只要上次额度刷新/主动探测已过窗口，仍应继续定期探测
    selected = _select_probe_key_ids(
        keys=keys,  # type: ignore[arg-type]
        provider_type="codex",
        now_ts=now_ts,
        interval_seconds=10 * 60,
        last_probe_timestamps={"k1": now_ts - (25 * 60)},
        limit=0,
    )
    assert selected == ["k1"]


def test_select_probe_key_ids_applies_limit_by_oldest_anchor_first() -> None:
    now = datetime(2026, 3, 5, 12, 0, 0, tzinfo=timezone.utc)
    now_ts = int(now.timestamp())

    keys = [
        _key("k1", upstream_metadata={"codex": {"updated_at": now_ts - (60 * 60)}}),
        _key("k2", upstream_metadata={"codex": {"updated_at": now_ts - (50 * 60)}}),
        _key("k3", upstream_metadata={"codex": {"updated_at": now_ts - (40 * 60)}}),
    ]

    selected = _select_probe_key_ids(
        keys=keys,  # type: ignore[arg-type]
        provider_type="codex",
        now_ts=now_ts,
        interval_seconds=10 * 60,
        last_probe_timestamps={},
        limit=2,
    )
    assert selected == ["k1", "k2"]


@pytest.mark.asyncio
async def test_run_local_quota_check_for_provider_counts_hard_blocked_keys() -> None:
    scheduler = PoolQuotaProbeScheduler()
    db = MagicMock()
    db.query.return_value.options.return_value.filter.return_value.all.return_value = [
        _key("k1", upstream_metadata={"codex": {"primary_used_percent": 98.5}}),
        _key("k2", upstream_metadata={"codex": {"primary_used_percent": 40.0}}),
        _key("k3", upstream_metadata={"codex": {"secondary_used_percent": 100.0}}),
    ]

    result = await scheduler._run_local_quota_check_for_provider(
        db=db,
        provider_id="provider-1",
        provider_type="codex",
        key_ids=["k1", "k2", "k3"],
    )

    assert result == {
        "selected": 3,
        "hard_blocked": 2,
        "available": 1,
    }


@pytest.mark.asyncio
async def test_run_probe_cycle_uses_local_check_for_codex(monkeypatch: pytest.MonkeyPatch) -> None:
    scheduler = PoolQuotaProbeScheduler()
    scheduler.running = True

    provider = SimpleNamespace(
        id="provider-codex-1",
        provider_type="codex",
        config={"pool_advanced": {"probing_enabled": True, "probing_interval_minutes": 10}},
    )
    provider_db = MagicMock()
    provider_db.query.return_value.filter.return_value.all.return_value = [provider]
    probe_db = MagicMock()

    create_session_mock = MagicMock(side_effect=[provider_db, probe_db])
    monkeypatch.setattr(
        "src.services.provider_keys.pool_quota_probe_scheduler.create_session",
        create_session_mock,
    )
    monkeypatch.setattr(
        "src.services.provider_keys.pool_quota_probe_scheduler.get_redis_client",
        AsyncMock(return_value=None),
    )

    select_mock = AsyncMock(return_value=["k1", "k2"])
    mark_mock = AsyncMock()
    local_check_mock = AsyncMock(return_value={"selected": 2, "hard_blocked": 1, "available": 1})
    refresh_mock = AsyncMock()

    monkeypatch.setattr(scheduler, "_select_keys_for_provider", select_mock)
    monkeypatch.setattr(scheduler, "_mark_probe_timestamps", mark_mock)
    monkeypatch.setattr(scheduler, "_run_local_quota_check_for_provider", local_check_mock)
    monkeypatch.setattr(
        "src.services.provider_keys.pool_quota_probe_scheduler.refresh_provider_quota_for_provider",
        refresh_mock,
    )

    await scheduler._run_probe_cycle()

    select_mock.assert_awaited_once()
    mark_mock.assert_awaited_once()
    local_check_mock.assert_awaited_once_with(
        db=probe_db,
        provider_id="provider-codex-1",
        provider_type="codex",
        key_ids=["k1", "k2"],
    )
    refresh_mock.assert_not_awaited()
