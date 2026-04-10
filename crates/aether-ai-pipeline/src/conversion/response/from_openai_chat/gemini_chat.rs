use serde_json::{json, Value};

use super::shared::{
    build_generated_tool_call_id, extract_openai_assistant_text, parse_openai_function_arguments,
};

pub fn convert_openai_chat_response_to_gemini_chat(
    body_json: &Value,
    report_context: &Value,
) -> Option<Value> {
    let body = body_json.as_object()?;
    let choices = body.get("choices")?.as_array()?;
    let mut candidates = Vec::new();
    for choice in choices {
        let choice = choice.as_object()?;
        let message = choice.get("message")?.as_object()?;
        let mut parts = Vec::new();

        if let Some(reasoning_content) = message.get("reasoning_content").and_then(Value::as_str) {
            if !reasoning_content.trim().is_empty() {
                parts.push(json!({
                    "text": reasoning_content,
                    "thought": true,
                }));
            }
        }
        if let Some(text) = extract_openai_assistant_text(message.get("content")) {
            if !text.trim().is_empty() {
                parts.push(json!({ "text": text }));
            }
        }
        if let Some(tool_call_values) = message.get("tool_calls").and_then(Value::as_array) {
            for (index, tool_call) in tool_call_values.iter().enumerate() {
                let tool_call = tool_call.as_object()?;
                let function = tool_call.get("function")?.as_object()?;
                let tool_name = function
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?;
                let call_id = tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| build_generated_tool_call_id(index));
                parts.push(json!({
                    "functionCall": {
                        "id": call_id,
                        "name": tool_name,
                        "args": parse_openai_function_arguments(function.get("arguments"))?,
                    }
                }));
            }
        }
        if parts.is_empty() {
            parts.push(json!({ "text": "" }));
        }

        let mut finish_reason = match choice.get("finish_reason").and_then(Value::as_str) {
            Some("stop") | None => "STOP",
            Some("length") => "MAX_TOKENS",
            Some("content_filter") => "SAFETY",
            Some("tool_calls") | Some("function_call") => "STOP",
            Some(other) => other,
        };
        if parts.iter().any(|part| part.get("functionCall").is_some()) {
            finish_reason = "STOP";
        }
        candidates.push(json!({
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": finish_reason,
            "index": choice.get("index").and_then(Value::as_u64).unwrap_or(0),
        }));
    }
    let usage = body.get("usage").and_then(Value::as_object);
    let prompt_tokens = usage
        .and_then(|value| value.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .and_then(|value| value.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_tokens = usage
        .and_then(|value| value.get("completion_tokens_details"))
        .and_then(Value::as_object)
        .and_then(|details| details.get("reasoning_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visible_completion_tokens = completion_tokens.saturating_sub(reasoning_tokens);
    let total_tokens = usage
        .and_then(|value| value.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens + completion_tokens);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| report_context.get("mapped_model").and_then(Value::as_str))
        .or_else(|| report_context.get("model").and_then(Value::as_str))
        .unwrap_or("unknown");
    let response_id = body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("resp-local-finalize");

    Some(json!({
        "responseId": response_id,
        "modelVersion": model,
        "candidates": candidates,
        "usageMetadata": {
            "promptTokenCount": prompt_tokens,
            "candidatesTokenCount": visible_completion_tokens,
            "thoughtsTokenCount": reasoning_tokens,
            "totalTokenCount": total_tokens,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::convert_openai_chat_response_to_gemini_chat;
    use serde_json::json;

    #[test]
    fn preserves_multiple_openai_choices_and_reasoning_tokens_for_gemini() {
        let response = json!({
            "id": "chatcmpl_123",
            "model": "gpt-5.4",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "hello",
                        "reasoning_content": "step by step"
                    },
                    "finish_reason": "stop"
                },
                {
                    "index": 1,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "lookup",
                                "arguments": "{\"city\":\"Shanghai\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 7,
                "total_tokens": 17,
                "completion_tokens_details": {
                    "reasoning_tokens": 2
                }
            }
        });

        let converted = convert_openai_chat_response_to_gemini_chat(&response, &json!({}))
            .expect("response should convert");

        assert_eq!(
            converted["candidates"]
                .as_array()
                .expect("candidates")
                .len(),
            2
        );
        assert_eq!(
            converted["candidates"][0]["content"]["parts"][0]["thought"],
            true
        );
        assert_eq!(
            converted["candidates"][1]["content"]["parts"][0]["functionCall"]["name"],
            "lookup"
        );
        assert_eq!(converted["usageMetadata"]["candidatesTokenCount"], 5);
        assert_eq!(converted["usageMetadata"]["thoughtsTokenCount"], 2);
    }
}
