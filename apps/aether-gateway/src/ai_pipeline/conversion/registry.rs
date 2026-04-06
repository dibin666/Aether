use crate::ai_pipeline::provider_transport_facade::auth::{
    resolve_local_gemini_auth, resolve_local_openai_chat_auth, resolve_local_standard_auth,
};
use crate::ai_pipeline::provider_transport_facade::policy::{
    supports_local_openai_chat_transport, supports_local_standard_transport_with_network,
};
use crate::ai_pipeline::provider_transport_facade::{
    supports_local_gemini_transport_with_network, GatewayProviderTransportSnapshot,
};

pub(crate) use aether_ai_pipeline::conversion::{
    request_conversion_kind, sync_chat_response_conversion_kind, sync_cli_response_conversion_kind,
    RequestConversionKind, SyncChatResponseConversionKind, SyncCliResponseConversionKind,
};

pub(crate) fn request_conversion_transport_supported(
    transport: &GatewayProviderTransportSnapshot,
    _kind: RequestConversionKind,
) -> bool {
    match transport
        .endpoint
        .api_format
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "openai:chat" => supports_local_openai_chat_transport(transport),
        "openai:cli" => supports_local_standard_transport_with_network(transport, "openai:cli"),
        "openai:compact" => {
            supports_local_standard_transport_with_network(transport, "openai:compact")
        }
        "claude:chat" => supports_local_standard_transport_with_network(transport, "claude:chat"),
        "claude:cli" => supports_local_standard_transport_with_network(transport, "claude:cli"),
        "gemini:chat" => supports_local_gemini_transport_with_network(transport, "gemini:chat"),
        "gemini:cli" => supports_local_gemini_transport_with_network(transport, "gemini:cli"),
        _ => false,
    }
}

pub(crate) fn request_conversion_direct_auth(
    transport: &GatewayProviderTransportSnapshot,
    _kind: RequestConversionKind,
) -> Option<(String, String)> {
    match transport
        .endpoint
        .api_format
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "openai:chat" => resolve_local_openai_chat_auth(transport),
        "gemini:chat" | "gemini:cli" => resolve_local_gemini_auth(transport),
        "openai:cli" | "openai:compact" | "claude:chat" | "claude:cli" => {
            resolve_local_standard_auth(transport)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        request_conversion_kind, sync_chat_response_conversion_kind,
        sync_cli_response_conversion_kind, RequestConversionKind, SyncChatResponseConversionKind,
        SyncCliResponseConversionKind,
    };

    #[test]
    fn request_conversion_registry_supports_bidirectional_standard_matrix() {
        assert_eq!(
            request_conversion_kind("claude:chat", "openai:chat"),
            Some(RequestConversionKind::ToOpenAIChat)
        );
        assert_eq!(
            request_conversion_kind("gemini:chat", "claude:chat"),
            Some(RequestConversionKind::ToClaudeStandard)
        );
        assert_eq!(
            request_conversion_kind("gemini:cli", "openai:compact"),
            Some(RequestConversionKind::ToOpenAICompact)
        );
        assert_eq!(
            request_conversion_kind("openai:compact", "gemini:cli"),
            Some(RequestConversionKind::ToGeminiStandard)
        );
        assert_eq!(request_conversion_kind("claude:chat", "claude:chat"), None);
    }

    #[test]
    fn sync_response_conversion_registry_supports_bidirectional_standard_matrix() {
        assert_eq!(
            sync_chat_response_conversion_kind("openai:chat", "claude:chat"),
            Some(SyncChatResponseConversionKind::ToClaudeChat)
        );
        assert_eq!(
            sync_chat_response_conversion_kind("claude:chat", "gemini:chat"),
            Some(SyncChatResponseConversionKind::ToGeminiChat)
        );
        assert_eq!(
            sync_chat_response_conversion_kind("gemini:chat", "openai:chat"),
            Some(SyncChatResponseConversionKind::ToOpenAIChat)
        );
        assert_eq!(
            sync_cli_response_conversion_kind("openai:cli", "gemini:cli"),
            Some(SyncCliResponseConversionKind::ToGeminiCli)
        );
        assert_eq!(
            sync_cli_response_conversion_kind("claude:cli", "openai:compact"),
            Some(SyncCliResponseConversionKind::ToOpenAIFamilyCli)
        );
        assert_eq!(
            sync_cli_response_conversion_kind("gemini:cli", "claude:cli"),
            Some(SyncCliResponseConversionKind::ToClaudeCli)
        );
    }
}
