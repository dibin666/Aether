from __future__ import annotations

from datetime import datetime, timezone
from decimal import Decimal

from sqlalchemy import create_engine
from sqlalchemy.orm import Session, sessionmaker

from src.api.admin.pool.routes import (
    _build_pool_consumption_summary,
    _load_pool_key_consumption_rows,
    _resolve_pool_consumption_window_start_ts,
)
from src.api.admin.pool.schemas import PoolConsumptionAccount
from src.models.database import Usage


def test_build_pool_consumption_summary_computes_totals_averages_and_extremes() -> None:
    accounts = [
        PoolConsumptionAccount(
            key_id="key-1",
            key_name="Alpha",
            request_count=10,
            input_tokens=1000,
            output_tokens=2000,
            cache_tokens=500,
            total_tokens=3500,
            total_cost_usd="3.50000000",
        ),
        PoolConsumptionAccount(
            key_id="key-2",
            key_name="Beta",
            request_count=2,
            input_tokens=100,
            output_tokens=50,
            cache_tokens=25,
            total_tokens=175,
            total_cost_usd="0.10000000",
        ),
    ]

    summary = _build_pool_consumption_summary(accounts)

    assert summary.account_count == 2
    assert summary.request_count == 12
    assert summary.input_tokens == 1100
    assert summary.output_tokens == 2050
    assert summary.cache_tokens == 525
    assert summary.total_tokens == 3675
    assert summary.total_cost_usd == "3.60000000"
    assert summary.avg_request_count == 6
    assert summary.avg_input_tokens == 550
    assert summary.avg_output_tokens == 1025
    assert summary.avg_cache_tokens == 262
    assert summary.avg_total_tokens == 1838
    assert summary.avg_total_cost_usd == "1.80000000"
    assert summary.max_account is not None
    assert summary.max_account.key_id == "key-1"
    assert summary.min_account is not None
    assert summary.min_account.key_id == "key-2"


def test_build_pool_consumption_summary_returns_zero_summary_for_empty_accounts() -> None:
    summary = _build_pool_consumption_summary([])

    assert summary.account_count == 0
    assert summary.request_count == 0
    assert summary.total_tokens == 0
    assert summary.total_cost_usd == "0.00000000"
    assert summary.avg_total_cost_usd == "0.00000000"
    assert summary.max_account is None
    assert summary.min_account is None


def test_resolve_pool_consumption_window_start_ts_uses_latest_codex_window() -> None:
    metadata = {
        "codex": {
            "primary_reset_at": 1_710_000_000,
            "primary_window_minutes": 7 * 24 * 60,
            "secondary_reset_at": 1_709_820_000,
            "secondary_window_minutes": 5 * 60,
        }
    }

    result = _resolve_pool_consumption_window_start_ts("codex", metadata)

    assert result == 1_709_802_000


def test_load_pool_key_consumption_rows_respects_per_key_window_starts() -> None:
    engine = create_engine("sqlite:///:memory:")
    session_factory = sessionmaker(bind=engine)
    Usage.__table__.create(engine)

    db: Session = session_factory()
    try:
        db.add_all(
            [
                Usage(
                    id="usage-1",
                    request_id="req-1",
                    provider_name="Codex",
                    model="gpt-5",
                    provider_id="provider-1",
                    provider_api_key_id="key-1",
                    input_tokens=10,
                    output_tokens=5,
                    total_tokens=15,
                    total_cost_usd=Decimal("0.10000000"),
                    created_at=datetime(2026, 4, 20, 10, 0, tzinfo=timezone.utc),
                ),
                Usage(
                    id="usage-2",
                    request_id="req-2",
                    provider_name="Codex",
                    model="gpt-5",
                    provider_id="provider-1",
                    provider_api_key_id="key-1",
                    input_tokens=20,
                    output_tokens=10,
                    total_tokens=30,
                    total_cost_usd=Decimal("0.20000000"),
                    created_at=datetime(2026, 4, 20, 13, 0, tzinfo=timezone.utc),
                ),
                Usage(
                    id="usage-3",
                    request_id="req-3",
                    provider_name="Codex",
                    model="gpt-5",
                    provider_id="provider-1",
                    provider_api_key_id="key-2",
                    input_tokens=5,
                    output_tokens=5,
                    total_tokens=10,
                    total_cost_usd=Decimal("0.05000000"),
                    created_at=datetime(2026, 4, 20, 9, 0, tzinfo=timezone.utc),
                ),
                Usage(
                    id="usage-4",
                    request_id="req-4",
                    provider_name="Codex",
                    model="gpt-5",
                    provider_id="provider-1",
                    provider_api_key_id="key-2",
                    input_tokens=15,
                    output_tokens=10,
                    total_tokens=25,
                    total_cost_usd=Decimal("0.15000000"),
                    created_at=datetime(2026, 4, 20, 15, 0, tzinfo=timezone.utc),
                ),
            ]
        )
        db.commit()

        result = _load_pool_key_consumption_rows(
            db,
            provider_id="provider-1",
            key_ids=["key-1", "key-2"],
            time_range=None,
            usage_window_starts={
                "key-1": datetime(2026, 4, 20, 12, 0, tzinfo=timezone.utc),
            },
        )

        assert result["key-1"]["request_count"] == 1
        assert result["key-1"]["input_tokens"] == 20
        assert result["key-1"]["total_tokens"] == 30
        assert Decimal(str(result["key-1"]["total_cost_usd"])) == Decimal("0.20000000")

        assert result["key-2"]["request_count"] == 2
        assert result["key-2"]["input_tokens"] == 20
        assert result["key-2"]["total_tokens"] == 35
        assert Decimal(str(result["key-2"]["total_cost_usd"])) == Decimal("0.20000000")
    finally:
        db.close()
        engine.dispose()
