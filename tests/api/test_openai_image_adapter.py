from __future__ import annotations

from fastapi.responses import JSONResponse

from src.api.handlers.openai.adapter import OpenAIChatAdapter
from src.api.handlers.openai_cli.adapter import OpenAIImageAdapter


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
