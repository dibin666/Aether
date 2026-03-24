use base64::Engine as _;

use super::super::submission::{
    maybe_build_local_core_error_response, resolve_core_error_background_report_kind,
    spawn_sync_finalize, spawn_sync_report, submit_sync_finalize, submit_sync_report,
};
use super::*;
use crate::gateway::video_tasks::VideoTaskSyncReportMode;

#[path = "execution/policy.rs"]
mod policy;
#[path = "execution/response.rs"]
mod response;

use policy::{
    decode_execution_result_body, resolve_core_sync_error_finalize_report_kind,
    should_fallback_to_control_sync, should_finalize_sync_response,
};
use response::{
    maybe_build_local_sync_finalize_response, maybe_build_local_video_error_response,
    maybe_build_local_video_success_outcome,
};

pub(crate) use response::{
    resolve_local_sync_error_background_report_kind,
    resolve_local_sync_success_background_report_kind,
};

#[allow(clippy::too_many_arguments)] // internal function, grouping would add unnecessary indirection
pub(super) async fn execute_executor_sync(
    state: &AppState,
    control_base_url: &str,
    executor_base_url: &str,
    request_path: &str,
    plan: ExecutionPlan,
    trace_id: &str,
    decision: &GatewayControlDecision,
    plan_kind: &str,
    report_kind: Option<String>,
    report_context: Option<serde_json::Value>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let plan_request_id = plan.request_id.as_str();
    let plan_candidate_id = plan.candidate_id.as_deref();
    let response = match state
        .client
        .post(format!("{executor_base_url}/v1/execute/sync"))
        .header(TRACE_ID_HEADER, trace_id)
        .json(&plan)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            warn!(trace_id = %trace_id, error = %err, "gateway direct executor sync unavailable");
            return Ok(None);
        }
    };

    if response.status() != http::StatusCode::OK {
        return Ok(Some(attach_control_metadata_headers(
            build_client_response(response, trace_id, Some(decision))?,
            Some(plan_request_id),
            plan_candidate_id,
        )?));
    }

    let result: ExecutionResult = response
        .json()
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let request_id = (!result.request_id.trim().is_empty())
        .then_some(result.request_id.as_str())
        .or(Some(plan_request_id));
    let candidate_id = result.candidate_id.as_deref().or(plan_candidate_id);
    let mut headers = result.headers.clone();
    let (body_bytes, body_json, body_base64) = decode_execution_result_body(&result, &mut headers)?;
    let has_body_bytes = body_base64.is_some();
    let explicit_finalize = should_finalize_sync_response(report_kind.as_deref());
    let mapped_error_finalize_kind =
        resolve_core_sync_error_finalize_report_kind(plan_kind, &result, body_json.as_ref());
    let finalize_report_kind = if explicit_finalize {
        report_kind.clone()
    } else {
        mapped_error_finalize_kind.clone()
    };

    if should_fallback_to_control_sync(
        plan_kind,
        &result,
        body_json.as_ref(),
        has_body_bytes,
        explicit_finalize,
        mapped_error_finalize_kind.is_some(),
    ) {
        return Ok(None);
    }

    if let Some(finalize_report_kind) = finalize_report_kind {
        let payload = GatewaySyncReportRequest {
            trace_id: trace_id.to_string(),
            report_kind: finalize_report_kind,
            report_context,
            status_code: result.status_code,
            headers: headers.clone(),
            body_json: body_json.clone(),
            client_body_json: None,
            body_base64: body_base64.clone(),
            telemetry: result.telemetry.clone(),
        };
        if let Some(outcome) =
            maybe_build_local_core_sync_finalize_response(trace_id, decision, &payload)?
        {
            if let Some(report_payload) = outcome.background_report {
                spawn_sync_report(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    report_payload,
                );
            } else {
                spawn_sync_finalize(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    payload,
                );
            }
            return Ok(Some(attach_control_metadata_headers(
                outcome.response,
                request_id,
                candidate_id,
            )?));
        }
        if let Some(outcome) = maybe_build_local_video_success_outcome(
            trace_id,
            decision,
            &payload,
            &state.video_tasks,
            &plan,
        )? {
            if let Some(snapshot) = outcome.local_task_snapshot.clone() {
                state.video_tasks.record_snapshot(snapshot);
            }
            match outcome.report_mode {
                VideoTaskSyncReportMode::InlineSync => {
                    submit_sync_report(state, control_base_url, trace_id, outcome.report_payload)
                        .await?;
                }
                VideoTaskSyncReportMode::Background => {
                    spawn_sync_report(
                        state.clone(),
                        control_base_url.to_string(),
                        trace_id.to_string(),
                        outcome.report_payload,
                    );
                }
            }
            return Ok(Some(attach_control_metadata_headers(
                outcome.response,
                request_id,
                candidate_id,
            )?));
        }
        if let Some(response) =
            maybe_build_local_sync_finalize_response(trace_id, decision, &payload)?
        {
            state
                .video_tasks
                .apply_finalize_mutation(request_path, payload.report_kind.as_str());
            if let Some(success_report_kind) =
                resolve_local_sync_success_background_report_kind(payload.report_kind.as_str())
            {
                let mut report_payload = payload.clone();
                report_payload.report_kind = success_report_kind;
                spawn_sync_report(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    report_payload,
                );
            } else {
                spawn_sync_finalize(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    payload,
                );
            }
            return Ok(Some(attach_control_metadata_headers(
                response,
                request_id,
                candidate_id,
            )?));
        }
        if let Some(response) =
            maybe_build_local_video_error_response(trace_id, decision, &payload)?
        {
            if let Some(error_report_kind) =
                resolve_local_sync_error_background_report_kind(payload.report_kind.as_str())
            {
                let mut report_payload = payload.clone();
                report_payload.report_kind = error_report_kind;
                spawn_sync_report(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    report_payload,
                );
            } else {
                spawn_sync_finalize(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    payload,
                );
            }
            return Ok(Some(attach_control_metadata_headers(
                response,
                request_id,
                candidate_id,
            )?));
        }
        if let Some(response) = maybe_build_local_core_error_response(trace_id, decision, &payload)?
        {
            if let Some(error_report_kind) =
                resolve_core_error_background_report_kind(payload.report_kind.as_str())
            {
                let mut report_payload = payload.clone();
                report_payload.report_kind = error_report_kind;
                spawn_sync_report(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    report_payload,
                );
            } else {
                spawn_sync_finalize(
                    state.clone(),
                    control_base_url.to_string(),
                    trace_id.to_string(),
                    payload,
                );
            }
            return Ok(Some(attach_control_metadata_headers(
                response,
                request_id,
                candidate_id,
            )?));
        }
        let response = submit_sync_finalize(state, control_base_url, trace_id, payload).await?;
        return Ok(Some(attach_control_metadata_headers(
            build_client_response(response, trace_id, Some(decision))?,
            request_id,
            candidate_id,
        )?));
    }

    if let Some(report_kind) = report_kind {
        let report = GatewaySyncReportRequest {
            trace_id: trace_id.to_string(),
            report_kind,
            report_context,
            status_code: result.status_code,
            headers: headers.clone(),
            body_json: body_json.clone(),
            client_body_json: None,
            body_base64: body_base64.clone(),
            telemetry: result.telemetry.clone(),
        };
        spawn_sync_report(
            state.clone(),
            control_base_url.to_string(),
            trace_id.to_string(),
            report,
        );
    }

    if let Some(request_id) = request_id.map(str::trim).filter(|value| !value.is_empty()) {
        headers.insert(
            CONTROL_REQUEST_ID_HEADER.to_string(),
            request_id.to_string(),
        );
    }

    if let Some(candidate_id) = candidate_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert(
            CONTROL_CANDIDATE_ID_HEADER.to_string(),
            candidate_id.to_string(),
        );
    }

    Ok(Some(build_client_response_from_parts(
        result.status_code,
        &headers,
        Body::from(body_bytes),
        trace_id,
        Some(decision),
    )?))
}
