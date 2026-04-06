use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use url::form_urlencoded;

use super::codex::apply_codex_openai_cli_special_body_edits;
use crate::ai_pipeline::conversion::{request_conversion_kind, RequestConversionKind};
use crate::ai_pipeline::planner::transport_facade::GatewayProviderTransportSnapshot;
use crate::ai_pipeline::provider_transport_facade::antigravity::{
    build_antigravity_v1internal_url, AntigravityRequestUrlAction,
};
use crate::ai_pipeline::provider_transport_facade::apply_local_body_rules;
use crate::ai_pipeline::provider_transport_facade::url::{
    build_claude_messages_url, build_gemini_content_url, build_openai_chat_url,
    build_openai_cli_url, build_passthrough_path_url,
};
use aether_ai_pipeline::planner::standard::normalize::{
    build_cross_format_openai_chat_request_body as pipeline_build_cross_format_openai_chat_request_body,
    build_cross_format_openai_cli_request_body as pipeline_build_cross_format_openai_cli_request_body,
    build_local_openai_chat_request_body as pipeline_build_local_openai_chat_request_body,
    build_local_openai_cli_request_body as pipeline_build_local_openai_cli_request_body,
};

pub(crate) fn build_local_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    upstream_is_stream: bool,
    body_rules: Option<&Value>,
) -> Option<Value> {
    let mut provider_request_body =
        pipeline_build_local_openai_chat_request_body(body_json, mapped_model, upstream_is_stream)?;
    if !apply_local_body_rules(&mut provider_request_body, body_rules, Some(body_json)) {
        return None;
    }
    Some(provider_request_body)
}

pub(crate) fn build_local_openai_chat_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
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
        None => Some(build_openai_chat_url(
            &transport.endpoint.base_url,
            parts.uri.query(),
        )),
    }
}

pub(crate) fn build_cross_format_openai_chat_request_body(
    body_json: &Value,
    mapped_model: &str,
    provider_type: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
) -> Option<Value> {
    let mut provider_request_body = pipeline_build_cross_format_openai_chat_request_body(
        body_json,
        mapped_model,
        provider_api_format,
        upstream_is_stream,
    )?;
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

pub(crate) fn build_cross_format_openai_chat_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<String> {
    let conversion_kind = request_conversion_kind("openai:chat", provider_api_format)?;
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
        None => match conversion_kind {
            RequestConversionKind::ToClaudeStandard => Some(build_claude_messages_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
            )),
            RequestConversionKind::ToGeminiStandard => build_gemini_content_url(
                &transport.endpoint.base_url,
                mapped_model,
                upstream_is_stream,
                parts.uri.query(),
            ),
            RequestConversionKind::ToOpenAIFamilyCli => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                false,
            )),
            RequestConversionKind::ToOpenAICompact => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                true,
            )),
            _ => None,
        },
    }
}

pub(crate) fn build_local_openai_cli_request_body(
    body_json: &Value,
    mapped_model: &str,
    require_streaming: bool,
    provider_type: &str,
    provider_api_format: &str,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
) -> Option<Value> {
    let mut provider_request_body =
        pipeline_build_local_openai_cli_request_body(body_json, mapped_model, require_streaming)?;
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

pub(crate) fn build_cross_format_openai_cli_request_body(
    body_json: &Value,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
    provider_type: &str,
    body_rules: Option<&Value>,
    user_api_key_id: Option<&str>,
) -> Option<Value> {
    let mut provider_request_body = pipeline_build_cross_format_openai_cli_request_body(
        body_json,
        mapped_model,
        client_api_format,
        provider_api_format,
        upstream_is_stream,
    )?;
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

pub(crate) fn build_local_openai_cli_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    compact: bool,
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
        None => Some(build_openai_cli_url(
            &transport.endpoint.base_url,
            parts.uri.query(),
            compact,
        )),
    }
}

pub(crate) fn build_cross_format_openai_cli_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
    client_api_format: &str,
    provider_api_format: &str,
    upstream_is_stream: bool,
) -> Option<String> {
    let conversion_kind = request_conversion_kind(client_api_format, provider_api_format)?;
    if transport
        .provider
        .provider_type
        .trim()
        .eq_ignore_ascii_case("antigravity")
    {
        let query = parts.uri.query().map(|query| {
            form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect::<BTreeMap<String, String>>()
        });
        return build_antigravity_v1internal_url(
            &transport.endpoint.base_url,
            if upstream_is_stream {
                AntigravityRequestUrlAction::StreamGenerateContent
            } else {
                AntigravityRequestUrlAction::GenerateContent
            },
            query.as_ref(),
        );
    }

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
        None => match conversion_kind {
            RequestConversionKind::ToOpenAIFamilyCli => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                false,
            )),
            RequestConversionKind::ToOpenAICompact => Some(build_openai_cli_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
                true,
            )),
            RequestConversionKind::ToClaudeStandard => Some(build_claude_messages_url(
                &transport.endpoint.base_url,
                parts.uri.query(),
            )),
            RequestConversionKind::ToGeminiStandard => build_gemini_content_url(
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
            "openai",
            None,
            None,
        )
        .expect("compact to openai cli body should build");

        assert_eq!(provider_request_body["model"], "gpt-5-upstream");
        assert_eq!(provider_request_body["input"][0]["type"], "message");
        assert_eq!(provider_request_body["input"][0]["role"], "user");
    }

    #[test]
    fn strips_metadata_for_codex_openai_cli_requests() {
        let body_json = json!({
            "model": "claude-sonnet-4-5",
            "metadata": {"trace_id": "abc"},
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            }],
        });

        let provider_request_body = build_cross_format_openai_cli_request_body(
            &body_json,
            "gpt-5-upstream",
            "claude:cli",
            "openai:cli",
            true,
            "codex",
            None,
            None,
        )
        .expect("claude cli to codex request should build");

        assert!(provider_request_body.get("metadata").is_none());
    }

    #[test]
    fn applies_codex_defaults_unless_body_rules_handle_the_field() {
        let body_json = json!({
            "model": "claude-sonnet-4-5",
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            }],
            "metadata": {"trace_id": "abc"},
            "store": true
        });
        let body_rules = json!([
            {"action":"set","path":"store","value":true},
            {"action":"set","path":"instructions","value":"Custom instructions"},
            {"action":"set","path":"metadata","value":{"trace_id":"keep-me"}}
        ]);

        let provider_request_body = build_cross_format_openai_cli_request_body(
            &body_json,
            "gpt-5-upstream",
            "claude:cli",
            "openai:cli",
            true,
            "codex",
            Some(&body_rules),
            None,
        )
        .expect("claude cli to codex request should build");

        assert_eq!(provider_request_body["store"], true);
        assert_eq!(provider_request_body["instructions"], "Custom instructions");
        assert_eq!(provider_request_body["metadata"]["trace_id"], "keep-me");
    }

    #[test]
    fn injects_codex_prompt_cache_key_for_openai_cli_cross_format_requests() {
        let body_json = json!({
            "model": "claude-sonnet-4-5",
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            }],
        });

        let provider_request_body = build_cross_format_openai_cli_request_body(
            &body_json,
            "gpt-5-upstream",
            "claude:cli",
            "openai:cli",
            true,
            "codex",
            None,
            Some("key-123"),
        )
        .expect("claude cli to codex request should build");

        assert_eq!(
            provider_request_body["prompt_cache_key"],
            "172c39e6-c0a0-5a70-8b63-e0f8e0d185a3"
        );
    }

    #[test]
    fn injects_codex_prompt_cache_key_for_openai_chat_cross_format_requests() {
        let body_json = json!({
            "model": "gpt-5",
            "messages": [{
                "role": "user",
                "content": "hello"
            }],
        });

        let provider_request_body = super::build_cross_format_openai_chat_request_body(
            &body_json,
            "gpt-5-upstream",
            "codex",
            "openai:cli",
            false,
            None,
            Some("key-123"),
        )
        .expect("openai chat to codex request should build");

        assert_eq!(
            provider_request_body["prompt_cache_key"],
            "172c39e6-c0a0-5a70-8b63-e0f8e0d185a3"
        );
    }
}
