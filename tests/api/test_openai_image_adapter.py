from __future__ import annotations

from unittest.mock import Mock

from fastapi.responses import JSONResponse

from src.api.handlers.openai.adapter import OpenAIChatAdapter
from src.api.handlers.openai_cli.adapter import OpenAIImageAdapter
from src.api.handlers.openai_cli.handler import OpenAIImageMessageHandler


def _build_image_handler() -> OpenAIImageMessageHandler:
    return OpenAIImageMessageHandler(
        db=Mock(),
        user=Mock(),
        api_key=Mock(),
        request_id="req-image",
        client_ip="127.0.0.1",
        user_agent="pytest",
        start_time=0.0,
    )


def test_openai_image_adapter_normalizes_defaults() -> None:
    adapter = OpenAIImageAdapter()

    body = {"prompt": "draw a lake"}
    normalized = adapter._normalize_image_request_body(body)

    assert isinstance(normalized, dict)
    assert normalized["model"] == "gpt-image-2"
    assert normalized["n"] == 1
    assert normalized["response_format"] == "b64_json"


def test_openai_image_adapter_rejects_non_gpt_image_2_and_n_gt_one() -> None:
    adapter = OpenAIImageAdapter()

    wrong_model = adapter._normalize_image_request_body({"prompt": "draw", "model": "gpt-5"})
    assert isinstance(wrong_model, JSONResponse)
    assert wrong_model.status_code == 400

    wrong_n = adapter._normalize_image_request_body(
        {"prompt": "draw", "model": "gpt-image-2", "n": 2}
    )
    assert isinstance(wrong_n, JSONResponse)
    assert wrong_n.status_code == 400


def test_openai_chat_adapter_rejects_gpt_image_2() -> None:
    adapter = OpenAIChatAdapter()

    result = adapter._validate_request_body(
        {
            "model": "gpt-image-2",
            "messages": [{"role": "user", "content": "hello"}],
        }
    )

    assert isinstance(result, JSONResponse)
    assert result.status_code == 400


def test_openai_image_handler_prepares_codex_internal_request_body() -> None:
    handler = _build_image_handler()

    requested_model, routing_model, dispatch = handler.prepare_request_for_dispatch(
        {
            "model": "gpt-image-2",
            "prompt": "draw a lake",
            "size": "1024x1024",
            "quality": "medium",
            "response_format": "b64_json",
            "n": 1,
        }
    )

    assert requested_model == "gpt-image-2"
    assert routing_model == "gpt-image-2"
    assert dispatch["model"] == "gpt-5.4"
    assert dispatch["input"] == [{"role": "user", "content": "draw a lake"}]
    assert dispatch["instructions"] == "you are a helpful assistant"
    assert dispatch["store"] is False
    assert dispatch["tools"] == [
        {"type": "image_generation", "size": "1024x1024", "quality": "medium"}
    ]


def test_openai_image_handler_finalize_provider_request_keeps_internal_model() -> None:
    handler = _build_image_handler()

    finalized = handler.finalize_provider_request(
        {
            "model": "gpt-image-2",
            "input": [{"role": "user", "content": "draw a lake"}],
            "tools": [{"type": "image_generation"}],
        },
        mapped_model="gpt-image-2",
        provider_api_format="openai:image",
    )

    assert finalized["model"] == "gpt-5.4"
