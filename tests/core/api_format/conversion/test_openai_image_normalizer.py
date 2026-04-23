from __future__ import annotations

from src.core.api_format.conversion.normalizers.openai_image import OpenAIImageNormalizer
from src.core.api_format.conversion.stream_bridge import InternalStreamAggregator
from src.core.api_format.conversion.stream_state import StreamState
from src.core.api_format.conversion.internal import ImageBlock


def test_openai_image_request_roundtrip_builds_responses_tool_request() -> None:
    normalizer = OpenAIImageNormalizer()

    internal = normalizer.request_to_internal(
        {
            "prompt": "A poster about Chinese history",
            "size": "1024x1024",
            "quality": "medium",
        }
    )

    assert internal.model == "gpt-image-2"
    out = normalizer.request_from_internal(internal)

    assert out["model"] == "gpt-image-2"
    assert out["tool_choice"] == "auto"
    assert out["tools"][0]["type"] == "image_generation"
    assert out["tools"][0]["size"] == "1024x1024"
    assert out["tools"][0]["quality"] == "medium"
    assert out["input"][0]["content"][0] == {
        "type": "input_text",
        "text": "A poster about Chinese history",
    }


def test_openai_image_response_uses_tool_usage_and_emits_images_payload() -> None:
    normalizer = OpenAIImageNormalizer()

    internal = normalizer.response_to_internal(
        {
            "id": "resp_img_123",
            "object": "response",
            "model": "gpt-5.4",
            "status": "completed",
            "output": [
                {
                    "id": "ig_123",
                    "type": "image_generation_call",
                    "output_format": "png",
                    "revised_prompt": "revised history prompt",
                    "result": "aGVsbG8=",
                }
            ],
            "usage": {"input_tokens": 2440, "output_tokens": 184, "total_tokens": 2624},
            "tool_usage": {
                "image_gen": {
                    "input_tokens": 171,
                    "output_tokens": 1372,
                    "total_tokens": 1543,
                }
            },
        }
    )

    assert internal.usage is not None
    assert internal.usage.input_tokens == 171
    assert internal.usage.output_tokens == 1372
    assert isinstance(internal.content[0], ImageBlock)
    assert internal.content[0].data == "aGVsbG8="
    assert internal.content[0].extra["revised_prompt"] == "revised history prompt"

    out = normalizer.response_from_internal(internal)
    assert out["data"][0]["b64_json"] == "aGVsbG8="
    assert out["data"][0]["revised_prompt"] == "revised history prompt"
    assert out["usage"] == {
        "input_tokens": 171,
        "output_tokens": 1372,
        "total_tokens": 1543,
    }


def test_openai_image_stream_chunk_to_internal_aggregates_image_result() -> None:
    normalizer = OpenAIImageNormalizer()
    state = StreamState(model="gpt-image-2", message_id="req-img")
    aggregator = InternalStreamAggregator(fallback_id="req-img", fallback_model="gpt-image-2")

    chunks = [
        {
            "type": "response.created",
            "response": {"id": "resp_img_123", "model": "gpt-5.4", "status": "in_progress"},
        },
        {
            "type": "response.output_item.done",
            "output_index": 0,
            "item": {
                "id": "ig_123",
                "type": "image_generation_call",
                "status": "generating",
                "output_format": "png",
                "revised_prompt": "history prompt",
                "result": "aGVsbG8=",
            },
        },
        {
            "type": "response.completed",
            "response": {
                "id": "resp_img_123",
                "model": "gpt-5.4",
                "status": "completed",
                "tool_usage": {
                    "image_gen": {
                        "input_tokens": 10,
                        "output_tokens": 20,
                        "total_tokens": 30,
                    }
                },
            },
        },
    ]

    for chunk in chunks:
        aggregator.feed(normalizer.stream_chunk_to_internal(chunk, state))

    internal = aggregator.build()
    assert internal.id == "resp_img_123"
    assert internal.usage is not None
    assert internal.usage.total_tokens == 30
    assert isinstance(internal.content[0], ImageBlock)
    assert internal.content[0].data == "aGVsbG8="
