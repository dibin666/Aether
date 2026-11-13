use crate::ai_serving::{convert_request, FormatContext};

use super::{
    resolve_native_provider_candidate_payload_parts, AppState, GatewayError,
    LocalSameFormatProviderCandidateAttempt, LocalSameFormatProviderCandidatePayloadParts,
    LocalSameFormatProviderDecisionInput, LocalSameFormatProviderSpec,
};

pub(super) async fn resolve_decisions_conversion_candidate(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    body: &serde_json::Value,
    body_base64: Option<&str>,
    input: &LocalSameFormatProviderDecisionInput,
    attempt: &LocalSameFormatProviderCandidateAttempt,
    client_spec: LocalSameFormatProviderSpec,
) -> Result<Option<LocalSameFormatProviderCandidatePayloadParts>, GatewayError> {
    let provider_format = attempt.eligible.provider_api_format.as_str();
    if !crate::ai_serving::transport::request_conversion_enabled_for_transport(
        &attempt.eligible.transport,
        client_spec.api_format,
        provider_format,
    ) {
        return Ok(None);
    }
    let plan = match provider_format {
        "gemini:decisions" => "gemini_decisions_sync",
        "openai:decisions" => "openai_decisions_sync",
        "systemone:decisions" => "systemone_decisions_sync",
        _ => return Ok(None),
    };
    let provider_spec = crate::ai_serving::resolve_local_same_format_sync_spec(plan)
        .expect("Decisions provider has a sync spec");
    let mapped_model =
        crate::ai_serving::planner::candidate_preparation::resolve_candidate_mapped_model(
            &attempt.eligible.candidate,
        )
        .map_err(|reason| GatewayError::Client {
            status: http::StatusCode::BAD_REQUEST,
            message: format!("Decisions conversion: {reason}"),
        })?;
    let converted = convert_request(
        client_spec.api_format,
        provider_format,
        body,
        &FormatContext::default().with_mapped_model(mapped_model),
    )
    .map_err(|error| GatewayError::Client {
        status: http::StatusCode::BAD_REQUEST,
        message: format!("Decisions conversion: {error}"),
    })?;
    let mut resolved = resolve_native_provider_candidate_payload_parts(
        state,
        parts,
        trace_id,
        &converted,
        body_base64,
        input,
        attempt,
        provider_spec,
    )
    .await?;
    if let Some(resolved) = resolved.as_mut() {
        resolved.report_kind = client_spec.report_kind;
    }
    Ok(resolved)
}
