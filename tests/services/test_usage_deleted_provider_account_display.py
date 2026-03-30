from __future__ import annotations

from datetime import date, datetime
from types import SimpleNamespace
from typing import Any, cast

from src.core.enums import UserRole
from src.services.analytics.query_service import AnalyticsFilters, AnalyticsQueryService
from src.services.system.time_range import TimeRangeParams


class _SequentialQuery:
    def __init__(self, result: list[Any]) -> None:
        self._result = result

    def filter(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
        return self

    def order_by(self, *_args: object, **_kwargs: object) -> "_SequentialQuery":
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


class _FakeRecordsQuery:
    def __init__(self, rows: list[Any]) -> None:
        self._rows = rows
        self._offset = 0
        self._limit = len(rows)

    def filter(self, *_args: object, **_kwargs: object) -> "_FakeRecordsQuery":
        return self

    def with_entities(self, *_args: object, **_kwargs: object) -> "_FakeRecordsQuery":
        return self

    def scalar(self) -> int:
        return len(self._rows)

    def order_by(self, *_args: object, **_kwargs: object) -> "_FakeRecordsQuery":
        return self

    def offset(self, value: int) -> "_FakeRecordsQuery":
        self._offset = value
        return self

    def limit(self, value: int) -> "_FakeRecordsQuery":
        self._limit = value
        return self

    def all(self) -> list[Any]:
        return self._rows[self._offset : self._offset + self._limit]


def _empty_filters() -> AnalyticsFilters:
    return AnalyticsFilters(
        user_ids=[],
        provider_names=[],
        models=[],
        target_models=[],
        api_key_ids=[],
        api_formats=[],
        request_types=[],
        statuses=[],
        error_categories=[],
        is_stream=None,
        has_format_conversion=None,
    )


def _usage_row(**overrides: Any) -> SimpleNamespace:
    base = dict(
        id="usage-1",
        request_id="req-1",
        created_at=datetime(2026, 3, 30, 12, 0, 0),
        user_id="user-1",
        username="dibin",
        api_key_id="key-1",
        api_key_name="User-Key",
        provider_api_key_id="provider-key-1",
        provider_name="codex",
        model="gpt-5.4",
        target_model=None,
        api_format="openai:cli",
        request_type="responses",
        status="completed",
        billing_status="completed",
        is_stream=False,
        has_format_conversion=False,
        status_code=200,
        error_message=None,
        error_category=None,
        response_time_ms=1234,
        first_byte_time_ms=321,
        input_tokens=100,
        output_tokens=50,
        input_output_total_tokens=150,
        cache_creation_input_tokens=0,
        cache_creation_input_tokens_5m=0,
        cache_creation_input_tokens_1h=0,
        cache_read_input_tokens=0,
        input_context_tokens=100,
        total_tokens=150,
        input_cost_usd=0.1,
        output_cost_usd=0.2,
        cache_creation_cost_usd=0.0,
        cache_creation_cost_usd_5m=0.0,
        cache_creation_cost_usd_1h=0.0,
        cache_read_cost_usd=0.0,
        cache_cost_usd=0.0,
        request_cost_usd=0.0,
        total_cost_usd=0.3,
        actual_total_cost_usd=0.3,
        actual_cache_cost_usd=0.0,
        rate_multiplier=1.0,
        request_metadata=None,
    )
    base.update(overrides)
    return SimpleNamespace(**base)


def test_records_keep_existing_provider_api_key_name_when_key_still_exists(monkeypatch) -> None:
    monkeypatch.setattr(
        AnalyticsQueryService,
        "build_usage_query",
        lambda *_args, **_kwargs: _FakeRecordsQuery([_usage_row()]),
    )
    monkeypatch.setattr(
        AnalyticsQueryService,
        "_load_request_execution_flags",
        lambda *_args, **_kwargs: ({}, {}),
    )

    db = _SequentialSession(
        results=[
            [("user-1", "dibin")],
            [("key-1", "User-Key")],
            [("provider-key-1", "Pool-Key-A")],
            [],
        ]
    )

    result = AnalyticsQueryService.records(
        cast(Any, db),
        SimpleNamespace(id="admin-1", role=UserRole.ADMIN),
        time_range=TimeRangeParams(start_date=date(2026, 3, 30), end_date=date(2026, 3, 30)),
        scope_kind="global",
        scope_user_id=None,
        scope_api_key_id=None,
        filters=_empty_filters(),
        search=SimpleNamespace(text=None, request_id=None),
        limit=20,
        offset=0,
    )

    assert result["records"][0]["provider_api_key_name"] == "Pool-Key-A"
    assert result["records"][0]["provider_api_key_deleted"] is False


def test_records_fill_deleted_provider_account_email_from_delete_log(monkeypatch) -> None:
    monkeypatch.setattr(
        AnalyticsQueryService,
        "build_usage_query",
        lambda *_args, **_kwargs: _FakeRecordsQuery([_usage_row()]),
    )
    monkeypatch.setattr(
        AnalyticsQueryService,
        "_load_request_execution_flags",
        lambda *_args, **_kwargs: ({}, {}),
    )

    db = _SequentialSession(
        results=[
            [("user-1", "dibin")],
            [("key-1", "User-Key")],
            [],
            [("provider-key-1", "demo-account@example.com")],
        ]
    )

    result = AnalyticsQueryService.records(
        cast(Any, db),
        SimpleNamespace(id="admin-1", role=UserRole.ADMIN),
        time_range=TimeRangeParams(start_date=date(2026, 3, 30), end_date=date(2026, 3, 30)),
        scope_kind="global",
        scope_user_id=None,
        scope_api_key_id=None,
        filters=_empty_filters(),
        search=SimpleNamespace(text=None, request_id=None),
        limit=20,
        offset=0,
    )

    assert result["records"][0]["provider_api_key_name"] == "demo-account@example.com"
    assert result["records"][0]["provider_api_key_deleted"] is True


def test_records_do_not_mark_deleted_when_delete_log_has_no_email(monkeypatch) -> None:
    monkeypatch.setattr(
        AnalyticsQueryService,
        "build_usage_query",
        lambda *_args, **_kwargs: _FakeRecordsQuery([_usage_row()]),
    )
    monkeypatch.setattr(
        AnalyticsQueryService,
        "_load_request_execution_flags",
        lambda *_args, **_kwargs: ({}, {}),
    )

    db = _SequentialSession(
        results=[
            [("user-1", "dibin")],
            [("key-1", "User-Key")],
            [],
            [("provider-key-1", None)],
        ]
    )

    result = AnalyticsQueryService.records(
        cast(Any, db),
        SimpleNamespace(id="admin-1", role=UserRole.ADMIN),
        time_range=TimeRangeParams(start_date=date(2026, 3, 30), end_date=date(2026, 3, 30)),
        scope_kind="global",
        scope_user_id=None,
        scope_api_key_id=None,
        filters=_empty_filters(),
        search=SimpleNamespace(text=None, request_id=None),
        limit=20,
        offset=0,
    )

    assert result["records"][0]["provider_api_key_name"] is None
    assert result["records"][0]["provider_api_key_deleted"] is False
