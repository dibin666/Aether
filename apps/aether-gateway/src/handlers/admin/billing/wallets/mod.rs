use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::{body::Body, response::Response};

mod mutations;
mod reads;
mod routes;
mod shared;

pub(crate) async fn maybe_build_local_admin_wallets_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_body: Option<&axum::body::Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    routes::maybe_build_local_admin_wallets_routes_response(state, request_context, request_body)
        .await
}
