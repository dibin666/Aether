"""OpenAI Images client normalizer backed by Responses/image_generation upstream."""

from __future__ import annotations

import time
from typing import Any

from src.core.api_format.conversion.internal import (
    ContentBlock,
    ContentType,
    ImageBlock,
    InternalMessage,
    InternalRequest,
    InternalResponse,
    Role,
    StopReason,
    TextBlock,
    UsageInfo,
)
from src.core.api_format.conversion.stream_events import (
    ContentBlockStartEvent,
    ContentBlockStopEvent,
    MessageStopEvent,
)
from src.core.api_format.conversion.stream_state import StreamState

from .openai_cli import OpenAICliNormalizer

_OPENAI_IMAGE_DEFAULT_MODEL = "gpt-image-2"
_TOOL_OPTION_KEYS = (
    "size",
    "quality",
    "background",
    "moderation",
    "output_compression",
    "output_format",
)


class OpenAIImageNormalizer(OpenAICliNormalizer):
    FORMAT_ID = "openai:image"

    def request_to_internal(self, request: dict[str, Any]) -> InternalRequest:
        prompt = str(request.get("prompt") or "")
        model = str(request.get("model") or _OPENAI_IMAGE_DEFAULT_MODEL)
        messages = [InternalMessage(role=Role.USER, content=[TextBlock(text=prompt)])]

        extra: dict[str, Any] = {
            "openai_image": {
                key: request[key]
                for key in (
                    "response_format",
                    "user",
                    "n",
                    *_TOOL_OPTION_KEYS,
                )
                if key in request
            }
        }
        extra["openai_image"]["prompt"] = prompt
        return InternalRequest(
            model=model,
            messages=messages,
            stream=False,
            n=self._optional_int(request.get("n")),
            extra=extra,
        )

    def request_from_internal(
        self,
        internal: InternalRequest,
        *,
        target_variant: str | None = None,
    ) -> dict[str, Any]:
        del target_variant
        image_extra = internal.extra.get("openai_image", {}) if internal.extra else {}
        tool: dict[str, Any] = {"type": "image_generation"}
        if isinstance(image_extra, dict):
            for key in _TOOL_OPTION_KEYS:
                value = image_extra.get(key)
                if value is not None:
                    tool[key] = value

        result: dict[str, Any] = {
            "model": internal.model or _OPENAI_IMAGE_DEFAULT_MODEL,
            "input": self._internal_messages_to_input(
                internal.messages,
                system_to_developer=False,
            ),
            "tools": [tool],
            "tool_choice": "auto",
        }
        if internal.instructions:
            instructions_text = self._join_instructions(internal.instructions)
            if instructions_text:
                result["instructions"] = instructions_text
        elif internal.system:
            result["instructions"] = internal.system
        return result

    def response_to_internal(self, response: dict[str, Any]) -> InternalResponse:
        payload = self._unwrap_response_object(response)
        rid = str(payload.get("id") or "")
        model = str(payload.get("model") or "")
        blocks = self._extract_image_blocks(payload)
        usage = self._extract_image_usage(payload)
        status = str(payload.get("status") or "")
        stop_reason = StopReason.END_TURN if status == "completed" else StopReason.UNKNOWN
        return InternalResponse(
            id=rid,
            model=model,
            content=blocks,
            stop_reason=stop_reason,
            usage=usage,
            extra={"openai_image": {"response": payload}},
        )

    def response_from_internal(
        self,
        internal: InternalResponse,
        *,
        requested_model: str | None = None,
    ) -> dict[str, Any]:
        del requested_model
        data: list[dict[str, Any]] = []
        for block in internal.content:
            if not isinstance(block, ImageBlock):
                continue
            item: dict[str, Any] = {}
            if block.data:
                item["b64_json"] = block.data
            elif block.url:
                item["url"] = block.url
            revised_prompt = block.extra.get("revised_prompt") if isinstance(block.extra, dict) else None
            if revised_prompt is not None:
                item["revised_prompt"] = revised_prompt
            data.append(item)

        result: dict[str, Any] = {
            "created": int(time.time()),
            "data": data,
        }
        usage = internal.usage or UsageInfo()
        if usage.input_tokens or usage.output_tokens or usage.total_tokens or usage.extra:
            result["usage"] = {
                "input_tokens": int(usage.input_tokens or 0),
                "output_tokens": int(usage.output_tokens or 0),
                "total_tokens": int(
                    usage.total_tokens or ((usage.input_tokens or 0) + (usage.output_tokens or 0))
                ),
            }
        return result

    def _handle_output_item_done(
        self, chunk: dict[str, Any], state: StreamState, ss: dict[str, Any]
    ) -> list[Any]:
        item = chunk.get("item")
        if not isinstance(item, dict):
            return []
        item_type = item.get("type")
        if item_type != "image_generation_call":
            return super()._handle_output_item_done(chunk, state, ss)

        image_data = str(item.get("result") or "").strip()
        if not image_data:
            return []

        output_format = str(item.get("output_format") or "png").strip().lower() or "png"
        media_type = _output_format_to_media_type(output_format)
        block_index = self._allocate_block_index(ss)
        extra = {
            "image_data": image_data,
            "image_media_type": media_type,
        }
        revised_prompt = item.get("revised_prompt")
        if revised_prompt is not None:
            extra["revised_prompt"] = revised_prompt
        return [
            ContentBlockStartEvent(
                block_index=block_index,
                block_type=ContentType.IMAGE,
                extra=extra,
            ),
            ContentBlockStopEvent(block_index=block_index),
        ]

    def _handle_response_completed(
        self, chunk: dict[str, Any], state: StreamState, ss: dict[str, Any]
    ) -> list[Any]:
        resp_obj = chunk.get("response")
        resp_obj = resp_obj if isinstance(resp_obj, dict) else {}
        usage = self._extract_image_usage(resp_obj)
        return [MessageStopEvent(stop_reason=StopReason.END_TURN, usage=usage)]

    _CHUNK_HANDLERS = dict(OpenAICliNormalizer._CHUNK_HANDLERS)
    _CHUNK_HANDLERS["response.output_item.done"] = _handle_output_item_done
    _CHUNK_HANDLERS["response.completed"] = _handle_response_completed

    def _extract_image_blocks(self, payload: dict[str, Any]) -> list[ContentBlock]:
        blocks: list[ContentBlock] = []
        output = payload.get("output")
        if not isinstance(output, list):
            return blocks
        for item in output:
            if not isinstance(item, dict) or item.get("type") != "image_generation_call":
                continue
            image_data = str(item.get("result") or "").strip()
            if not image_data:
                continue
            output_format = str(item.get("output_format") or "png").strip().lower() or "png"
            block = ImageBlock(
                data=image_data,
                media_type=_output_format_to_media_type(output_format),
                extra={
                    "revised_prompt": item.get("revised_prompt"),
                    "output_format": output_format,
                },
            )
            blocks.append(block)
        return blocks

    def _extract_image_usage(self, payload: dict[str, Any]) -> UsageInfo:
        tool_usage = payload.get("tool_usage")
        if isinstance(tool_usage, dict):
            image_usage = tool_usage.get("image_gen") or tool_usage.get("image_generation")
            if isinstance(image_usage, dict):
                return self._usage_to_internal(image_usage)
        return self._usage_to_internal(payload.get("usage"))



def _output_format_to_media_type(output_format: str) -> str:
    normalized = str(output_format or "").strip().lower()
    return {
        "jpg": "image/jpeg",
        "jpeg": "image/jpeg",
        "webp": "image/webp",
        "gif": "image/gif",
    }.get(normalized, "image/png")


__all__ = ["OpenAIImageNormalizer"]
