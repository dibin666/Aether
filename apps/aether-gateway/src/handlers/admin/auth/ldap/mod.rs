use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::{
    body::{Body, Bytes},
    response::Response,
};

mod builders;
mod routes;
mod shared;

pub(crate) async fn maybe_build_local_admin_ldap_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_body: Option<&Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    routes::maybe_build_local_admin_ldap_response(state, request_context, request_body).await
}
