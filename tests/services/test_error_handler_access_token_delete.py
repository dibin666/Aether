from __future__ import annotations

from types import SimpleNamespace
from typing import Any

import httpx
import pytest

from src.core.exceptions import ProviderAuthException, UpstreamClientException
from src.services.orchestration.error_handler import ErrorHandlerService


@pytest.mark.asyncio
async def test_handle_http_error_triggers_access_token_delete_for_http400(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    service = ErrorHandlerService(db=SimpleNamespace())
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    endpoint = SimpleNamespace(api_family='openai', endpoint_kind='cli')
    key = SimpleNamespace(
        id='k1',
        provider_id='p1',
        auth_type='oauth',
        proxy={'node_id': 'node-1', 'name': 'CF-1'},
    )

    request = httpx.Request('POST', 'https://example.test/v1/chat/completions')
    response = httpx.Response(400, request=request, text='bad request')
    http_error = httpx.HTTPStatusError('400', request=request, response=response)
    converted = UpstreamClientException(
        message='bad request',
        provider_name='Codex Pool',
        status_code=400,
        upstream_error='bad request',
    )

    called: dict[str, Any] = {}

    async def _fake_delete(**kwargs: Any) -> bool:
        called.update(kwargs)
        return True

    monkeypatch.setattr('src.services.orchestration.error_handler.delete_access_token_only_key_on_http400', _fake_delete)

    await service.handle_http_error(
        http_error=http_error,
        converted_error=converted,
        error_response_text='bad request',
        provider=provider,
        endpoint=endpoint,
        key=key,
        affinity_key='affinity-1',
        api_format='openai:cli',
        global_model_id='gpt-5.2',
        request_id='req-1',
        captured_key_concurrent=None,
    )

    assert called['key_id'] == 'k1'
    assert called['status_code'] == 400
    assert called['endpoint_sig'] == 'openai:cli'
    assert called['request_id'] == 'req-1'


@pytest.mark.asyncio
async def test_handle_http_error_skips_access_token_delete_for_non_400(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    service = ErrorHandlerService(db=SimpleNamespace())
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    endpoint = SimpleNamespace(api_family='openai', endpoint_kind='cli')
    key = SimpleNamespace(id='k1', provider_id='p1', auth_type='oauth', proxy=None)

    request = httpx.Request('POST', 'https://example.test/v1/chat/completions')
    response = httpx.Response(401, request=request, text='unauthorized')
    http_error = httpx.HTTPStatusError('401', request=request, response=response)
    converted = ProviderAuthException(provider_name='Codex Pool')

    called = {'count': 0}

    async def _fake_delete(**kwargs: Any) -> bool:
        called['count'] += 1
        return True

    monkeypatch.setattr('src.services.orchestration.error_handler.delete_access_token_only_key_on_http400', _fake_delete)
    monkeypatch.setattr(
        'src.services.orchestration.error_handler.get_health_monitor',
        lambda: SimpleNamespace(record_failure=lambda **kwargs: None),
    )

    await service.handle_http_error(
        http_error=http_error,
        converted_error=converted,
        error_response_text='unauthorized',
        provider=provider,
        endpoint=endpoint,
        key=key,
        affinity_key='affinity-1',
        api_format='openai:cli',
        global_model_id='gpt-5.2',
        request_id='req-1',
        captured_key_concurrent=None,
    )

    assert called['count'] == 0
