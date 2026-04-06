use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::{body::Body, response::Response};

mod analytics_routes;
mod cost_routes;
mod helpers;
mod leaderboard;
mod leaderboard_routes;
mod provider_quota_routes;
mod range;
mod responses;
mod timeseries;
pub(crate) use self::helpers::{round_to, AdminStatsTimeRange, AdminStatsUsageFilter};
pub(crate) use self::range::{list_usage_for_optional_range, parse_bounded_u32};
pub(crate) use self::responses::admin_stats_bad_request_response;
pub(crate) use self::timeseries::aggregate_usage_stats;

pub(crate) async fn maybe_build_local_admin_stats_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(decision) = request_context.control_decision.as_ref() else {
        return Ok(None);
    };
    if decision.route_family.as_deref() != Some("stats_manage") {
        return Ok(None);
    }

    if let Some(response) =
        provider_quota_routes::maybe_build_local_admin_stats_provider_quota_response(
            state,
            request_context,
            decision,
        )
        .await?
    {
        return Ok(Some(response));
    }

    if let Some(response) = analytics_routes::maybe_build_local_admin_stats_analytics_response(
        state,
        request_context,
        decision,
    )
    .await?
    {
        return Ok(Some(response));
    }

    if let Some(response) =
        cost_routes::maybe_build_local_admin_stats_cost_response(state, request_context).await?
    {
        return Ok(Some(response));
    }

    if let Some(response) = leaderboard_routes::maybe_build_local_admin_stats_leaderboard_response(
        state,
        request_context,
    )
    .await?
    {
        return Ok(Some(response));
    }

    Ok(None)
}
