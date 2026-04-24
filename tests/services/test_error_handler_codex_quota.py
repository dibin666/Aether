from __future__ import annotations

from types import SimpleNamespace
from typing import Any, cast
from unittest.mock import AsyncMock

import httpx
import pytest

from src.core.exceptions import ProviderRateLimitException
from src.services.orchestration.error_handler import ErrorHandlerService


class _FakeDB:
    def __init__(self) -> None:
        self.added: list[object] = []
        self.commit_count = 0

    def add(self, obj: object) -> None:
        self.added.append(obj)

    def commit(self) -> None:
        self.commit_count += 1


@pytest.mark.asyncio
async def test_handle_http_error_records_codex_usage_limit_metadata(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    db = _FakeDB()
    service = ErrorHandlerService(db=cast(Any, db))
    service.handle_rate_limit = AsyncMock()  # type: ignore[method-assign]

    monkeypatch.setattr(
        "src.services.orchestration.error_handler.get_health_monitor",
        lambda: SimpleNamespace(record_failure=lambda **_kwargs: None),
    )
    monkeypatch.setattr(
        "src.services.orchestration.error_handler.reset_access_token_only_key_http400_counter",
        lambda **_kwargs: None,
    )

    key = SimpleNamespace(id="k1", upstream_metadata={})
    provider = SimpleNamespace(provider_type="codex", name="Codex")
    endpoint = SimpleNamespace(id="e1", api_family="openai", endpoint_kind="cli")
    error_text = (
        '{"error":{"type":"usage_limit_reached","message":"The usage limit has been '
        'reached","resets_at":1777546708}}'
    )
    request = httpx.Request("POST", "https://example.test")
    response = httpx.Response(429, request=request, text=error_text)
    http_error = httpx.HTTPStatusError("rate limited", request=request, response=response)
    converted_error = ProviderRateLimitException(
        message="请求过于频繁，请稍后重试",
        provider_name="Codex",
        response_headers={},
        retry_after=None,
    )

    await service.handle_http_error(
        http_error,
        converted_error,
        error_text,
        provider=cast(Any, provider),
        endpoint=cast(Any, endpoint),
        key=cast(Any, key),
        affinity_key="affinity-1",
        api_format="openai:chat",
        global_model_id="gpt-5.4",
        request_id="req-1",
        captured_key_concurrent=None,
    )

    assert key.upstream_metadata["codex"]["quota_exhausted"] is True
    assert key.upstream_metadata["codex"]["quota_reset_at"] == 1777546708
    assert db.commit_count == 1
