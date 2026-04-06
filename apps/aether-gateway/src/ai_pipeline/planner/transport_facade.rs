pub(crate) use crate::ai_pipeline::provider_transport_facade::{
    GatewayProviderTransportSnapshot, LocalResolvedOAuthRequestAuth,
};
use crate::{AppState, GatewayError};

pub(crate) async fn read_provider_transport_snapshot(
    state: &AppState,
    provider_id: &str,
    endpoint_id: &str,
    key_id: &str,
) -> Result<Option<GatewayProviderTransportSnapshot>, GatewayError> {
    state
        .read_provider_transport_snapshot(provider_id, endpoint_id, key_id)
        .await
}

pub(crate) async fn resolve_local_oauth_request_auth(
    state: &AppState,
    transport: &GatewayProviderTransportSnapshot,
) -> Result<Option<LocalResolvedOAuthRequestAuth>, GatewayError> {
    state.resolve_local_oauth_request_auth(transport).await
}
