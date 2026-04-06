use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::{body::Body, response::Response};

mod activity;
mod cache;
mod cache_affinity;
mod cache_affinity_reads;
mod cache_config;
mod cache_identity;
mod cache_model_mapping;
mod cache_mutations;
mod cache_payloads;
mod cache_route_helpers;
mod cache_store;
mod cache_types;
mod resilience;
mod responses;
mod route_filters;
mod routes;
#[cfg(test)]
pub(crate) mod test_support;
mod trace;
mod usage_helpers;

pub(crate) async fn maybe_build_local_admin_monitoring_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
) -> Result<Option<Response<Body>>, GatewayError> {
    routes::maybe_build_local_admin_monitoring_response(state, request_context).await
}

#[cfg(test)]
mod tests;
