from __future__ import annotations

from types import SimpleNamespace
from typing import Any

import httpx
import pytest

from src.services.orchestration.error_classifier import ErrorClassifier


@pytest.mark.asyncio
async def test_error_classifier_runs_http400_delete_side_effect_for_upstream_client_error() -> None:
    classifier = ErrorClassifier(db=SimpleNamespace())
    provider = SimpleNamespace(id='p1', name='Codex Pool', provider_type='codex')
    endpoint = SimpleNamespace(api_family='openai', endpoint_kind='cli')
    key = SimpleNamespace(id='k1', provider_id='p1', auth_type='oauth', proxy=None)
    request = httpx.Request('POST', 'https://example.test/v1/chat/completions')
    response = httpx.Response(
        400,
        request=request,
        text='{"error":{"type":"invalid_request_error","message":"bad request"}}',
    )
    http_error = httpx.HTTPStatusError('400', request=request, response=response)

    called: dict[str, Any] = {}

    async def _fake_handle_http_error(*args: Any, **kwargs: Any) -> None:
        called['args'] = args
        called['kwargs'] = kwargs

    classifier._error_handler = SimpleNamespace(handle_http_error=_fake_handle_http_error)

    extra = await classifier.handle_http_error(
        http_error,
        provider=provider,
        endpoint=endpoint,
        key=key,
        affinity_key='affinity-1',
        api_format='openai:cli',
        global_model_id='gpt-5.2',
        request_id='req-1',
        captured_key_concurrent=None,
        elapsed_ms=120,
        attempt=1,
        max_attempts=2,
    )

    assert called['kwargs']['key'] is key
    assert called['kwargs']['provider'] is provider
    assert called['kwargs']['api_format'] == 'openai:cli'
    assert extra['converted_error'].status_code == 400
