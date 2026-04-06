use axum::body::Body;
use axum::http::Response;

use crate::{usage::GatewaySyncReportRequest, GatewayError};

pub(crate) use crate::execution_runtime::{ConversionMode, ExecutionStrategy};

pub(crate) fn maybe_build_local_sync_finalize_response(
    trace_id: &str,
    decision: &crate::ai_pipeline::control_facade::GatewayControlDecision,
    payload: &GatewaySyncReportRequest,
) -> Result<Option<Response<Body>>, GatewayError> {
    crate::execution_runtime::maybe_build_local_sync_finalize_response(trace_id, decision, payload)
}
