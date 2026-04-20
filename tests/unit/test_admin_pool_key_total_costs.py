from __future__ import annotations

from decimal import Decimal
from typing import Any
from unittest.mock import MagicMock

from src.api.admin.pool.routes import _load_pool_key_total_costs, _serialize_money


class _DummyQuery:
    def __init__(self, result: list[tuple[Any, Any]]) -> None:
        self._result = result

    def filter(self, *args: Any, **kwargs: Any) -> "_DummyQuery":
        return self

    def group_by(self, *args: Any, **kwargs: Any) -> "_DummyQuery":
        return self

    def all(self) -> list[tuple[Any, Any]]:
        return self._result


def test_load_pool_key_total_costs_returns_usage_sum_mapping() -> None:
    db = MagicMock()
    db.query.return_value = _DummyQuery(
        [
            ("key-1", Decimal("12.34000000")),
            ("key-2", Decimal("0.50000000")),
            (None, Decimal("99.00000000")),
        ]
    )

    result = _load_pool_key_total_costs(db, ["key-1", "key-2"])

    assert result == {
        "key-1": Decimal("12.34000000"),
        "key-2": Decimal("0.50000000"),
    }


def test_serialize_money_uses_usage_sum_for_historical_pool_cost_fix() -> None:
    usage_totals = {"key-1": Decimal("12.34000000")}
    stored_total_cost = Decimal("0")

    assert _serialize_money(usage_totals.get("key-1", stored_total_cost)) == "12.34000000"
