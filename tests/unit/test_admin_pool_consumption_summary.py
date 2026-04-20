from __future__ import annotations

from src.api.admin.pool.routes import _build_pool_consumption_summary
from src.api.admin.pool.schemas import PoolConsumptionAccount


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
