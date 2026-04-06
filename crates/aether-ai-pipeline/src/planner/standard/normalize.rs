use serde_json::Value;

use crate::conversion::request::{
    convert_openai_chat_request_to_claude_request, convert_openai_chat_request_to_gemini_request,
    convert_openai_chat_request_to_openai_cli_request,
    normalize_openai_cli_request_to_openai_chat_request,
};
use crate::conversion::{request_conversion_kind, RequestConversionKind};

pub fn build_local_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let request_body_object = body_json.as_object()?;
    let mut provider_request_body = serde_json::Map::from_iter(
        request_body_object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    provider_request_body.insert("model".to_string(), Value::String(mapped_model.to_string()));
    if upstream_is_stream {
        provider_request_body.insert("stream".to_string(), Value::Bool(true));
    }
    Some(Value::Object(provider_request_body))
}

pub fn build_cross_format_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let conversion_kind = request_conversion_kind("openai:chat", provider_api_format)?;
    match conversion_kind {
        RequestConversionKind::ToClaudeStandard => convert_openai_chat_request_to_claude_request(
            body_json,
            mapped_model,
            upstream_is_stream,
        ),
        RequestConversionKind::ToGeminiStandard => convert_openai_chat_request_to_gemini_request(
            body_json,
            mapped_model,
            upstream_is_stream,
        ),
        RequestConversionKind::ToOpenAIFamilyCli => {
            convert_openai_chat_request_to_openai_cli_request(
                body_json,
                mapped_model,
                upstream_is_stream,
                false,
            )
        }
        RequestConversionKind::ToOpenAICompact => {
            convert_openai_chat_request_to_openai_cli_request(body_json, mapped_model, false, true)
        }
        _ => None,
    }
}

pub fn build_local_openai_cli_request_body(
    body_json: &Value,
    mapped_model: &str,
    require_streaming: bool,
) -> Option<Value> {
    let request_body_object = body_json.as_object()?;
    let mut provider_request_body = serde_json::Map::from_iter(
        request_body_object
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    provider_request_body.insert("model".to_string(), Value::String(mapped_model.to_string()));
    if require_streaming {
        provider_request_body.insert("stream".to_string(), Value::Bool(true));
    }
    Some(Value::Object(provider_request_body))
}

pub fn build_cross_format_openai_cli_request_body(
    body_json: &Value,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<Value> {
    let chat_like_request = normalize_openai_cli_request_to_openai_chat_request(body_json)?;
    let conversion_kind = request_conversion_kind(client_api_format, provider_api_format)?;
    match conversion_kind {
        RequestConversionKind::ToOpenAIFamilyCli => {
            convert_openai_chat_request_to_openai_cli_request(
                &chat_like_request,
                mapped_model,
                upstream_is_stream,
                false,
            )
        }
        RequestConversionKind::ToOpenAICompact => {
            convert_openai_chat_request_to_openai_cli_request(
                &chat_like_request,
                mapped_model,
                false,
                true,
            )
        }
        RequestConversionKind::ToClaudeStandard => convert_openai_chat_request_to_claude_request(
            &chat_like_request,
            mapped_model,
            upstream_is_stream,
        ),
        RequestConversionKind::ToGeminiStandard => convert_openai_chat_request_to_gemini_request(
            &chat_like_request,
            mapped_model,
            upstream_is_stream,
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::build_cross_format_openai_cli_request_body;
    use serde_json::json;

    #[test]
    fn builds_openai_family_cross_format_request_body_from_compact_source() {
        let body_json = json!({
            "model": "gpt-5",
            "input": "hello",
        });

        let provider_request_body = build_cross_format_openai_cli_request_body(
            &body_json,
            "gpt-5-upstream",
            "openai:compact",
            "openai:cli",
            false,
        )
        .expect("compact to openai cli body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["input"][0]["type"], "message");
        assert_eq!(provider_request_body["input"][0]["role"], "user");
    }
}
