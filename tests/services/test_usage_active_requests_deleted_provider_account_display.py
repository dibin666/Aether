from __future__ import annotations

from datetime import datetime, timezone
from types import SimpleNamespace
from typing import Any

from src.services.usage.service import UsageService


class _SequentialQuery:
    def __init__(self, result: list[Any]) -> None:
        self._result = result

    def add_columns(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def outerjoin(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def filter(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def order_by(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def limit(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def all(self) -> list[Any]:
        return self._result


class _SequentialSession:
    def __init__(self, results: list[list[Any]]) -> None:
        self._results = results
        self.calls = 0

    def query(self, *_entities: object) -> _SequentialQuery:
        result = self._results[self.calls]
        self.calls += 1
        return _SequentialQuery(result)


def _active_request_row(**overrides: Any) -> SimpleNamespace:
    base = dict(
        id="usage-1",
        status="streaming",
        input_tokens=12,
        output_tokens=34,
        cache_creation_input_tokens=0,
        cache_read_input_tokens=0,
        total_cost_usd=0.12,
        actual_total_cost_usd=0.12,
        rate_multiplier=1.0,
        response_time_ms=1234,
        first_byte_time_ms=234,
        created_at=datetime(2026, 3, 30, 13, 21, tzinfo=timezone.utc),
        provider_endpoint_id="endpoint-1",
        provider_api_key_id="provider-key-1",
        api_format="openai:cli",
        endpoint_api_format="openai:cli",
        has_format_conversion=False,
        target_model="gpt-5.4",
        provider_name="Codex",
        api_key_name="Pool-Key-A",
    )
    base.update(overrides)
    return SimpleNamespace(**base)


def test_get_active_requests_status_keeps_existing_provider_key_name() -> None:
    db = _SequentialSession(results=[[_active_request_row()], []])

    result = UsageService.get_active_requests_status(
        db=db,  # type: ignore[arg-type]
        ids=["usage-1"],
        include_admin_fields=True,
        maintain_status=False,
    )

    assert result == [
        {
            "id": "usage-1",
            "status": "streaming",
            "input_tokens": 12,
            "output_tokens": 34,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "cost": 0.12,
            "actual_cost": 0.12,
            "rate_multiplier": 1.0,
            "response_time_ms": 1234,
            "first_byte_time_ms": 234,
            "api_format": "openai:cli",
            "endpoint_api_format": "openai:cli",
            "has_format_conversion": False,
            "target_model": "gpt-5.4",
            "provider": "Codex",
            "api_key_name": "Pool-Key-A",
            "provider_api_key_deleted": False,
        }
    ]


def test_get_active_requests_status_fills_deleted_provider_account_email() -> None:
    db = _SequentialSession(
        results=[
            [[_active_request_row(api_key_name=None)]][0],
            [("provider-key-1", "deleted-account@example.com")],
        ]
    )

    result = UsageService.get_active_requests_status(
        db=db,  # type: ignore[arg-type]
        ids=["usage-1"],
        include_admin_fields=True,
        maintain_status=False,
    )

    assert result[0]["api_key_name"] == "deleted-account@example.com"
    assert result[0]["provider_api_key_deleted"] is True
