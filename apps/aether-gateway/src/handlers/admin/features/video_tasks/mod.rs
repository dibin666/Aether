use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::{body::Body, response::Response};

mod builders;
mod routes;

pub(crate) async fn maybe_build_local_admin_video_tasks_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
) -> Result<Option<Response<Body>>, GatewayError> {
    routes::maybe_build_local_admin_video_tasks_response(state, request_context).await
}
