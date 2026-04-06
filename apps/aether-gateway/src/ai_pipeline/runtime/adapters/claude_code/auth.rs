use crate::ai_pipeline::provider_transport_facade::auth::resolve_local_standard_auth;
use crate::ai_pipeline::provider_transport_facade::snapshot::GatewayProviderTransportSnapshot;
use crate::ai_pipeline::provider_transport_facade::supports_local_oauth_request_auth_resolution;

pub(crate) fn supports_local_claude_code_auth(
    transport: &GatewayProviderTransportSnapshot,
) -> bool {
    resolve_local_standard_auth(transport).is_some()
        || supports_local_oauth_request_auth_resolution(transport)
}
