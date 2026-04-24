from __future__ import annotations

import sys
import types
from types import SimpleNamespace
from typing import Any, cast
from unittest.mock import AsyncMock

import httpx
import pytest

from src.core.exceptions import EmbeddedErrorException, ProviderRateLimitException
from src.services.request.executor import ExecutionContext, ExecutionError
from src.services.task.execute.error_handler import TaskErrorOperationsService
from src.services.task.execute.pool import TaskPoolOperationsService


class _FakeDB:
    def __init__(self) -> None:
        self.added: list[object] = []
        self.commit_count = 0

    def add(self, obj: object) -> None:
        self.added.append(obj)

    def commit(self) -> None:
        self.commit_count += 1


def _install_proxy_resolver(monkeypatch: pytest.MonkeyPatch) -> None:
    module = types.ModuleType("src.services.proxy_node.resolver")
    setattr(module, "resolve_effective_proxy", lambda provider_proxy, key_proxy: None)

    async def _resolve_proxy_info_async(_proxy: Any) -> None:
        return None

    setattr(module, "resolve_proxy_info_async", _resolve_proxy_info_async)
    monkeypatch.setitem(sys.modules, "src.services.proxy_node.resolver", module)


def _build_context() -> ExecutionContext:
    return ExecutionContext(
        candidate_id="cand-1",
        candidate_index=0,
        provider_id="p1",
        endpoint_id="e1",
        key_id="k1",
        user_id=None,
        api_key_id=None,
        is_cached_user=False,
        elapsed_ms=12,
        concurrent_requests=1,
    )


@pytest.mark.asyncio
async def test_handle_candidate_error_breaks_on_embedded_codex_usage_limit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_proxy_resolver(monkeypatch)
    mark_failed_calls: list[dict[str, Any]] = []
    monkeypatch.setattr(
        "src.services.task.execute.error_handler.RequestCandidateService.mark_candidate_failed",
        lambda **kwargs: mark_failed_calls.append(kwargs),
    )

    db = _FakeDB()
    service = TaskErrorOperationsService(db=cast(Any, db), pool_ops=TaskPoolOperationsService())
    error_text = (
        '{"error":{"type":"usage_limit_reached","message":"The usage limit has been '
        'reached","resets_at":1777546708}}'
    )
    candidate = SimpleNamespace(
        provider=SimpleNamespace(name="Codex", provider_type="codex", proxy=None),
        endpoint=SimpleNamespace(id="e1"),
        key=SimpleNamespace(id="k1", upstream_metadata={}, proxy=None),
    )
    exec_err = ExecutionError(
        EmbeddedErrorException(
            provider_name="Codex",
            error_code=429,
            error_message=error_text,
        ),
        _build_context(),
    )
    error_classifier = SimpleNamespace(is_client_error=lambda _msg: False, RETRIABLE_ERRORS=tuple())

    result = await service.handle_candidate_error(
        exec_err=exec_err,
        candidate=candidate,
        candidate_record_id="cand-1",
        retry_index=0,
        max_retries_for_candidate=2,
        affinity_key="affinity-1",
        api_format="openai:chat",
        global_model_id="gpt-5.4",
        request_id="req-1",
        attempt=1,
        max_attempts=2,
        error_classifier=error_classifier,
    )

    assert result == "break"
    assert len(mark_failed_calls) == 1
    assert candidate.key.upstream_metadata["codex"]["quota_exhausted"] is True
    assert db.commit_count == 1


@pytest.mark.asyncio
async def test_handle_candidate_error_http_codex_usage_limit_skips_retry(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_proxy_resolver(monkeypatch)
    mark_failed_calls: list[dict[str, Any]] = []
    monkeypatch.setattr(
        "src.services.task.execute.error_handler.RequestCandidateService.mark_candidate_failed",
        lambda **kwargs: mark_failed_calls.append(kwargs),
    )

    db = _FakeDB()
    pool_ops = TaskPoolOperationsService()
    pool_ops.pool_on_error = AsyncMock()  # type: ignore[method-assign]
    service = TaskErrorOperationsService(db=cast(Any, db), pool_ops=pool_ops)
    error_text = (
        '{"error":{"type":"usage_limit_reached","message":"The usage limit has been '
        'reached","resets_at":1777546708}}'
    )
    candidate = SimpleNamespace(
        provider=SimpleNamespace(name="Codex", provider_type="codex", proxy=None),
        endpoint=SimpleNamespace(id="e1"),
        key=SimpleNamespace(id="k1", upstream_metadata={}, proxy=None),
    )
    request = httpx.Request("POST", "https://example.test")
    response = httpx.Response(429, request=request, text=error_text)
    exec_err = ExecutionError(
        httpx.HTTPStatusError("rate limited", request=request, response=response),
        _build_context(),
    )
    error_classifier = SimpleNamespace(
        RETRIABLE_ERRORS=tuple(),
        handle_http_error=AsyncMock(
            return_value={
                "converted_error": ProviderRateLimitException(
                    message="请求过于频繁，请稍后重试",
                    provider_name="Codex",
                    response_headers={},
                    retry_after=None,
                ),
                "error_response": error_text,
            }
        ),
    )

    result = await service.handle_candidate_error(
        exec_err=exec_err,
        candidate=candidate,
        candidate_record_id="cand-1",
        retry_index=0,
        max_retries_for_candidate=2,
        affinity_key="affinity-1",
        api_format="openai:chat",
        global_model_id="gpt-5.4",
        request_id="req-1",
        attempt=1,
        max_attempts=2,
        error_classifier=error_classifier,
    )

    assert result == "break"
    assert len(mark_failed_calls) == 1
