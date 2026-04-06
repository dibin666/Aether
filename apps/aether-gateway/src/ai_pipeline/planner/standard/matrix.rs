use super::codex::apply_codex_openai_cli_special_body_edits;
use crate::ai_pipeline::planner::transport_facade::GatewayProviderTransportSnapshot;
use crate::ai_pipeline::provider_transport_facade::apply_local_body_rules;
use crate::ai_pipeline::provider_transport_facade::url::{
    build_claude_messages_url, build_gemini_content_url, build_openai_chat_url,
    build_openai_cli_url, build_passthrough_path_url,
};
use aether_ai_pipeline::planner::matrix::build_standard_request_body_from_canonical;
pub(crate) use aether_ai_pipeline::planner::standard::normalize_standard_request_to_openai_chat_request;
use serde_json::{json, Value};

pub(crate) fn build_standard_request_body(
    body_json: &Value,
    client_api_format: &str,
    mapped_model: &str,
    provider_type: &str,
    provider_api_format: &str,
    request_path: &str,
    upstream_is_stream: bool,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
) -> Option<Value> {
    let canonical_request = normalize_standard_request_to_openai_chat_request(
        body_json,
        client_api_format,
        request_path,
    )?;
    if cfg!(test) {
        println!("canonical_request: {canonical_request:#?}");
    }
    let mut provider_request_body = build_standard_request_body_from_canonical(
        &canonical_request,
        mapped_model,
        provider_api_format,
        upstream_is_stream,
    )?;
    if cfg!(test) {
        println!("provider_request_body before rules: {provider_request_body:#?}");
    }

    if !apply_local_body_rules(&mut provider_request_body, body_rules, Some(body_json)) {
        return None;
    }
    apply_codex_openai_cli_special_body_edits(
        &mut provider_request_body,
        provider_type,
        provider_api_format,
        body_rules,
        user_api_key_id,
    );
    Some(provider_request_body)
}

pub(crate) fn build_standard_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<String> {
    let custom_path = transport
        .endpoint
        .custom_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match custom_path {
        Some(path) => {
            build_passthrough_path_url(&transport.endpoint.base_url, path, parts.uri.query(), &[])
        }
        None => match provider_api_format.trim().to_ascii_lowercase().as_str() {
            "openai:chat" => Some(build_openai_chat_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
            )),
            "openai:cli" => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                false,
            )),
            "openai:compact" => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                true,
            )),
            "claude:chat" | "claude:cli" => Some(build_claude_messages_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
            )),
            "gemini:chat" | "gemini:cli" => build_gemini_content_url(
                &transport.endpoint.base_url,
                mapped_model,
                upstream_is_stream,
                parts.uri.query(),
            ),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::build_standard_request_body;
    use serde_json::json;

    #[test]
    fn builds_openai_chat_request_from_claude_chat_source() {
        let request = json!({
            "model": "claude-3-7-sonnet",
            "system": "You are concise.",
            "messages": [
                {
                    "role": "user",
                    "content": [{"type": "text", "text": "Hello from Claude"}]
                }
            ],
            "max_tokens": 128
        });

        let converted = build_standard_request_body(
            &request,
            "claude:chat",
            "gpt-5",
            "openai",
            "openai:chat",
            "/v1/messages",
            false,
            None,
            None,
        )
        .expect("claude chat should convert to openai chat");

        assert_eq!(converted["model"], "gpt-5");
        assert_eq!(converted["messages"][0]["role"], "system");
        assert_eq!(converted["messages"][0]["content"], "You are concise.");
        assert_eq!(converted["messages"][1]["role"], "user");
        assert_eq!(converted["messages"][1]["content"], "Hello from Claude");
    }

    #[test]
    fn builds_claude_chat_request_from_gemini_chat_source() {
        let request = json!({
            "systemInstruction": {
                "parts": [{"text": "Be brief."}]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": "Hello from Gemini"}]
                }
            ]
        });

        let converted = build_standard_request_body(
            &request,
            "gemini:chat",
            "claude-sonnet-4-5",
            "anthropic",
            "claude:chat",
            "/v1beta/models/gemini-2.5-pro:generateContent",
            false,
            None,
            None,
        )
        .expect("gemini chat should convert to claude chat");

        assert_eq!(converted["model"], "claude-sonnet-4-5");
        assert_eq!(converted["messages"][0]["role"], "user");
        assert!(
            converted["messages"]
                .to_string()
                .contains("Hello from Gemini"),
            "converted claude payload should retain the gemini user text: {converted}"
        );
    }

    #[test]
    fn builds_gemini_cli_request_from_claude_cli_source() {
        let request = json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {
                    "role": "user",
                    "content": [{"type": "text", "text": "Need CLI output"}]
                }
            ],
            "max_tokens": 64
        });

        let converted = build_standard_request_body(
            &request,
            "claude:cli",
            "gemini-2.5-pro",
            "google",
            "gemini:cli",
            "/v1/messages",
            false,
            None,
            None,
        )
        .expect("claude cli should convert to gemini cli");

        assert_eq!(converted["contents"][0]["role"], "user");
        assert_eq!(
            converted["contents"][0]["parts"][0]["text"],
            "Need CLI output"
        );
    }

    #[test]
    fn builds_openai_cli_request_from_claude_cli_source_with_forced_stream() {
        let request = json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {
                    "role": "user",
                    "content": [{"type": "text", "text": "Need OpenAI CLI output"}]
                }
            ],
            "max_tokens": 64
        });

        let converted = build_standard_request_body(
            &request,
            "claude:cli",
            "gpt-5",
            "openai",
            "openai:cli",
            "/v1/messages",
            true,
            None,
            None,
        )
        .expect("claude cli should convert to openai cli");

        assert_eq!(converted["model"], "gpt-5");
        assert_eq!(converted["input"][0]["role"], "user");
        assert_eq!(converted["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(
            converted["input"][0]["content"][0]["text"],
            "Need OpenAI CLI output"
        );
        assert_eq!(converted["stream"], true);
    }

    #[test]
    fn strips_metadata_for_codex_openai_cli_requests() {
        let request = json!({
            "model": "claude-sonnet-4-5",
            "metadata": {"trace_id": "abc"},
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "Need OpenAI CLI output"}]
            }],
            "max_tokens": 64
        });

        let converted = build_standard_request_body(
            &request,
            "claude:cli",
            "gpt-5",
            "codex",
            "openai:cli",
            "/v1/messages",
            true,
            None,
            None,
        )
        .expect("claude cli should convert to codex cli");

        assert!(converted.get("metadata").is_none());
    }

    #[test]
    fn applies_codex_defaults_unless_body_rules_handle_the_field() {
        let request = json!({
            "model": "claude-sonnet-4-5",
            "metadata": {"trace_id": "abc"},
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "Need OpenAI CLI output"}]
            }],
            "max_tokens": 64
        });
        let body_rules = json!([
            {"action":"set","path":"store","value":true},
            {"action":"set","path":"instructions","value":"Custom instructions"},
            {"action":"set","path":"metadata","value":{"trace_id":"keep-me"}}
        ]);

        let converted = build_standard_request_body(
            &request,
            "claude:cli",
            "gpt-5",
            "codex",
            "openai:cli",
            "/v1/messages",
            true,
            Some(&body_rules),
            None,
        )
        .expect("claude cli should convert to codex cli");

        assert_eq!(converted["store"], true);
        assert_eq!(converted["instructions"], "Custom instructions");
        assert_eq!(converted["metadata"]["trace_id"], "keep-me");
    }

    #[test]
    fn injects_codex_prompt_cache_key_for_standard_requests() {
        let request = json!({
            "model": "claude-sonnet-4-5",
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "Need OpenAI CLI output"}]
            }],
            "max_tokens": 64
        });

        let converted = build_standard_request_body(
            &request,
            "claude:cli",
            "gpt-5",
            "codex",
            "openai:cli",
            "/v1/messages",
            true,
            None,
            Some("key-123"),
        )
        .expect("claude cli should convert to codex cli");

        assert_eq!(
            converted["prompt_cache_key"],
            "172c39e6-c0a0-5a70-8b63-e0f8e0d185a3"
        );
    }
}
