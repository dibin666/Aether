use axum::http::Uri;

use crate::{AppState, GatewayError};

pub(crate) use crate::control::{GatewayControlAuthContext, GatewayControlDecision};

pub(crate) async fn resolve_execution_runtime_auth_context(
    state: &AppState,
    decision: &GatewayControlDecision,
    headers: &http::HeaderMap,
    uri: &Uri,
    trace_id: &str,
) -> Result<Option<GatewayControlAuthContext>, GatewayError> {
    crate::control::resolve_execution_runtime_auth_context(state, decision, headers, uri, trace_id)
        .await
}

pub(crate) fn collect_control_headers(
    headers: &http::HeaderMap,
) -> std::collections::BTreeMap<String, String> {
    crate::headers::collect_control_headers(headers)
}

pub(crate) fn is_json_request(headers: &http::HeaderMap) -> bool {
    crate::headers::is_json_request(headers)
}
