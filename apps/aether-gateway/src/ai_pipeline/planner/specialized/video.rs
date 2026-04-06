use std::collections::BTreeMap;

use aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate;
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::ai_pipeline::control_facade::{
    collect_control_headers, GatewayControlAuthContext, GatewayControlDecision,
};
use crate::ai_pipeline::execution_facade::{ConversionMode, ExecutionStrategy};
use crate::ai_pipeline::planner::auth_snapshot_facade::{
    read_auth_api_key_snapshot, GatewayAuthApiKeySnapshot,
};
use crate::ai_pipeline::planner::candidate_affinity::prefer_local_tunnel_owner_candidates;
use crate::ai_pipeline::planner::candidate_runtime_facade::{
    persist_available_local_candidate, persist_skipped_local_candidate,
};
use crate::ai_pipeline::planner::common::EXECUTION_RUNTIME_SYNC_DECISION_ACTION;
use crate::ai_pipeline::planner::executor_facade::mark_unused_local_candidate_items;
use crate::ai_pipeline::planner::plan_builders::{
    build_passthrough_sync_plan_from_decision, LocalSyncPlanAndReport,
};
use crate::ai_pipeline::planner::scheduler_facade::list_selectable_candidates;
use crate::ai_pipeline::planner::transport_facade::{
    read_provider_transport_snapshot, GatewayProviderTransportSnapshot,
};
use crate::ai_pipeline::provider_transport_facade::auth::{
    build_passthrough_headers_with_auth, resolve_local_gemini_auth, resolve_local_openai_chat_auth,
};
use crate::ai_pipeline::provider_transport_facade::policy::{
    supports_local_gemini_transport_with_network, supports_local_standard_transport_with_network,
};
use crate::ai_pipeline::provider_transport_facade::url::{
    build_gemini_video_predict_long_running_url, build_passthrough_path_url,
};
use crate::ai_pipeline::provider_transport_facade::{
    apply_local_body_rules, apply_local_header_rules, resolve_transport_execution_timeouts,
    resolve_transport_proxy_snapshot_with_tunnel_affinity, resolve_transport_tls_profile,
};
use crate::clock::current_unix_secs;
use crate::{AppState, GatewayControlSyncDecisionResponse, GatewayError};
use aether_ai_pipeline::planner::specialized::video::{
    resolve_sync_spec, LocalVideoCreateFamily, LocalVideoCreateSpec,
};

#[derive(Debug, Clone)]
struct LocalVideoCreateDecisionInput {
    auth_context: GatewayControlAuthContext,
    requested_model: String,
    auth_snapshot: GatewayAuthApiKeySnapshot,
}

#[derive(Debug, Clone)]
struct LocalVideoCreateCandidateAttempt {
    candidate: SchedulerMinimalCandidateSelectionCandidate,
    candidate_index: u32,
    candidate_id: String,
}

pub(crate) async fn build_local_video_sync_plan_and_reports_for_kind(
    state: &AppState,
    parts: &http::request::Parts,
    body_json: &serde_json::Value,
    trace_id: &str,
    decision: &GatewayControlDecision,
    plan_kind: &str,
) -> Result<Vec<LocalSyncPlanAndReport>, GatewayError> {
    let Some(spec) = resolve_sync_spec(plan_kind) else {
        return Ok(Vec::new());
    };

    build_local_sync_plan_and_reports(state, parts, body_json, trace_id, decision, spec).await
}

pub(crate) async fn maybe_build_sync_local_video_decision_payload(
    state: &AppState,
    parts: &http::request::Parts,
    body_json: &serde_json::Value,
    trace_id: &str,
    decision: &GatewayControlDecision,
    plan_kind: &str,
) -> Result<Option<GatewayControlSyncDecisionResponse>, GatewayError> {
    let Some(spec) = resolve_sync_spec(plan_kind) else {
        return Ok(None);
    };

    let Some(input) = resolve_local_video_create_decision_input(
        state, parts, trace_id, decision, body_json, spec,
    )
    .await
    else {
        return Ok(None);
    };

    let candidates = match list_selectable_candidates(
        state,
        spec.api_format,
        &input.requested_model,
        false,
        Some(&input.auth_snapshot),
        current_unix_secs(),
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(err) => {
            warn!(
                trace_id = %trace_id,
                decision_kind = spec.decision_kind,
                error = ?err,
                "gateway local video decision scheduler selection failed"
            );
            return Ok(None);
        }
    };

    let attempts = materialize_local_video_create_candidate_attempts(
        state,
        trace_id,
        &input,
        candidates,
        spec.api_format,
    )
    .await;

    for attempt in attempts {
        if let Some(payload) = maybe_build_local_video_create_decision_payload_for_candidate(
            state, parts, body_json, trace_id, &input, attempt, spec,
        )
        .await
        {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}

async fn build_local_sync_plan_and_reports(
    state: &AppState,
    parts: &http::request::Parts,
    body_json: &serde_json::Value,
    trace_id: &str,
    decision: &GatewayControlDecision,
    spec: LocalVideoCreateSpec,
) -> Result<Vec<LocalSyncPlanAndReport>, GatewayError> {
    let Some(input) = resolve_local_video_create_decision_input(
        state, parts, trace_id, decision, body_json, spec,
    )
    .await
    else {
        return Ok(Vec::new());
    };

    let candidates = match list_selectable_candidates(
        state,
        spec.api_format,
        &input.requested_model,
        false,
        Some(&input.auth_snapshot),
        current_unix_secs(),
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(err) => {
            warn!(
                trace_id = %trace_id,
                decision_kind = spec.decision_kind,
                error = ?err,
                "gateway local video decision scheduler selection failed"
            );
            return Ok(Vec::new());
        }
    };

    let attempts = materialize_local_video_create_candidate_attempts(
        state,
        trace_id,
        &input,
        candidates,
        spec.api_format,
    )
    .await;

    let mut plans = Vec::new();
    for attempt in attempts {
        let Some(payload) = maybe_build_local_video_create_decision_payload_for_candidate(
            state, parts, body_json, trace_id, &input, attempt, spec,
        )
        .await
        else {
            continue;
        };

        match build_passthrough_sync_plan_from_decision(parts, payload) {
            Ok(Some(value)) => plans.push(value),
            Ok(None) => {}
            Err(err) => {
                warn!(
                    trace_id = %trace_id,
                    decision_kind = spec.decision_kind,
                    error = ?err,
                    "gateway local video sync decision plan build failed"
                );
            }
        }
    }

    Ok(plans)
}

async fn resolve_local_video_create_decision_input(
    state: &AppState,
    parts: &http::request::Parts,
    trace_id: &str,
    decision: &GatewayControlDecision,
    body_json: &serde_json::Value,
    spec: LocalVideoCreateSpec,
) -> Option<LocalVideoCreateDecisionInput> {
    let Some(auth_context) = decision.auth_context.clone().filter(|auth_context| {
        !auth_context.user_id.trim().is_empty() && !auth_context.api_key_id.trim().is_empty()
    }) else {
        return None;
    };

    let requested_model = match spec.family {
        LocalVideoCreateFamily::OpenAi => body_json
            .get("model")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)?,
        LocalVideoCreateFamily::Gemini => extract_gemini_video_model_from_path(parts.uri.path())?,
    };

    let auth_snapshot = match read_auth_api_key_snapshot(
        state,
        &auth_context.user_id,
        &auth_context.api_key_id,
        current_unix_secs(),
    )
    .await
    {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => return None,
        Err(err) => {
            warn!(
                trace_id = %trace_id,
                decision_kind = spec.decision_kind,
                error = ?err,
                "gateway local video decision auth snapshot read failed"
            );
            return None;
        }
    };

    Some(LocalVideoCreateDecisionInput {
        auth_context,
        requested_model,
        auth_snapshot,
    })
}

async fn maybe_build_local_video_create_decision_payload_for_candidate(
    state: &AppState,
    parts: &http::request::Parts,
    body_json: &serde_json::Value,
    trace_id: &str,
    input: &LocalVideoCreateDecisionInput,
    attempt: LocalVideoCreateCandidateAttempt,
    spec: LocalVideoCreateSpec,
) -> Option<GatewayControlSyncDecisionResponse> {
    let LocalVideoCreateCandidateAttempt {
        candidate,
        candidate_index,
        candidate_id,
    } = attempt;
    let transport = match read_provider_transport_snapshot(
        state,
        &candidate.provider_id,
        &candidate.endpoint_id,
        &candidate.key_id,
    )
    .await
    {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            mark_skipped_local_video_candidate(
                state,
                input,
                trace_id,
                &candidate,
                candidate_index,
                &candidate_id,
                "transport_snapshot_missing",
            )
            .await;
            return None;
        }
        Err(err) => {
            warn!(
                trace_id = %trace_id,
                decision_kind = spec.decision_kind,
                error = ?err,
                "gateway local video decision provider transport read failed"
            );
            mark_skipped_local_video_candidate(
                state,
                input,
                trace_id,
                &candidate,
                candidate_index,
                &candidate_id,
                "transport_snapshot_read_failed",
            )
            .await;
            return None;
        }
    };

    let transport_supported = match spec.family {
        LocalVideoCreateFamily::OpenAi => {
            supports_local_standard_transport_with_network(&transport, spec.api_format)
        }
        LocalVideoCreateFamily::Gemini => {
            supports_local_gemini_transport_with_network(&transport, spec.api_format)
        }
    };
    if !transport_supported {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "transport_unsupported",
        )
        .await;
        return None;
    }

    let auth = match spec.family {
        LocalVideoCreateFamily::OpenAi => resolve_local_openai_chat_auth(&transport),
        LocalVideoCreateFamily::Gemini => resolve_local_gemini_auth(&transport),
    };
    let Some((auth_header, auth_value)) = auth else {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "transport_auth_unavailable",
        )
        .await;
        return None;
    };

    let mapped_model = candidate.selected_provider_model_name.trim().to_string();
    if mapped_model.is_empty() {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "mapped_model_missing",
        )
        .await;
        return None;
    }

    let upstream_url = build_video_upstream_url(parts, &transport, &mapped_model, spec.family);
    let Some(upstream_url) = upstream_url else {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "upstream_url_missing",
        )
        .await;
        return None;
    };

    let Some(provider_request_body) = build_provider_request_body(
        body_json,
        spec.family,
        &mapped_model,
        transport.endpoint.body_rules.as_ref(),
    ) else {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "provider_request_body_missing",
        )
        .await;
        return None;
    };
    let mut provider_request_headers = build_passthrough_headers_with_auth(
        &parts.headers,
        &auth_header,
        &auth_value,
        &BTreeMap::new(),
    );
    if !apply_local_header_rules(
        &mut provider_request_headers,
        transport.endpoint.header_rules.as_ref(),
        &[&auth_header, "content-type"],
        &provider_request_body,
        Some(body_json),
    ) {
        mark_skipped_local_video_candidate(
            state,
            input,
            trace_id,
            &candidate,
            candidate_index,
            &candidate_id,
            "transport_header_rules_apply_failed",
        )
        .await;
        return None;
    }
    let proxy = resolve_transport_proxy_snapshot_with_tunnel_affinity(state, &transport).await;
    let tls_profile = resolve_transport_tls_profile(&transport);

    Some(GatewayControlSyncDecisionResponse {
        action: EXECUTION_RUNTIME_SYNC_DECISION_ACTION.to_string(),
        decision_kind: Some(spec.decision_kind.to_string()),
        execution_strategy: Some(ExecutionStrategy::LocalSameFormat.as_str().to_string()),
        conversion_mode: Some(ConversionMode::None.as_str().to_string()),
        request_id: Some(trace_id.to_string()),
        candidate_id: Some(candidate_id.clone()),
        provider_name: Some(transport.provider.name.clone()),
        provider_id: Some(candidate.provider_id.clone()),
        endpoint_id: Some(candidate.endpoint_id.clone()),
        key_id: Some(candidate.key_id.clone()),
        upstream_base_url: Some(transport.endpoint.base_url.clone()),
        upstream_url: Some(upstream_url),
        provider_request_method: Some(parts.method.to_string()),
        auth_header: Some(auth_header),
        auth_value: Some(auth_value),
        provider_api_format: Some(spec.api_format.to_string()),
        client_api_format: Some(spec.api_format.to_string()),
        provider_contract: Some(spec.api_format.to_string()),
        client_contract: Some(spec.api_format.to_string()),
        model_name: Some(input.requested_model.clone()),
        mapped_model: Some(mapped_model.clone()),
        prompt_cache_key: None,
        extra_headers: BTreeMap::new(),
        provider_request_headers,
        provider_request_body: Some(provider_request_body),
        provider_request_body_base64: None,
        content_type: parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        proxy,
        tls_profile,
        timeouts: resolve_transport_execution_timeouts(&transport),
        upstream_is_stream: false,
        report_kind: Some(spec.report_kind.to_string()),
        report_context: Some(json!({
            "user_id": input.auth_context.user_id.clone(),
            "api_key_id": input.auth_context.api_key_id.clone(),
            "request_id": trace_id,
            "candidate_id": candidate_id,
            "candidate_index": candidate_index,
            "retry_index": 0,
            "model": input.requested_model.clone(),
            "provider_name": transport.provider.name.clone(),
            "provider_id": candidate.provider_id.clone(),
            "endpoint_id": candidate.endpoint_id.clone(),
            "key_id": candidate.key_id.clone(),
            "provider_api_format": spec.api_format,
            "client_api_format": spec.api_format,
            "mapped_model": mapped_model,
            "original_headers": collect_control_headers(&parts.headers),
            "original_request_body": body_json,
            "has_envelope": false,
            "needs_conversion": false,
        })),
        auth_context: Some(input.auth_context.clone()),
    })
}

fn build_provider_request_body(
    body_json: &serde_json::Value,
    family: LocalVideoCreateFamily,
    mapped_model: &str,
    body_rules: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut provider_request_body = match family {
        LocalVideoCreateFamily::OpenAi => {
            let mut provider_request_body = body_json.as_object().cloned().unwrap_or_default();
            provider_request_body
                .insert("model".to_string(), Value::String(mapped_model.to_string()));
            serde_json::Value::Object(provider_request_body)
        }
        LocalVideoCreateFamily::Gemini => body_json.clone(),
    };
    if !apply_local_body_rules(&mut provider_request_body, body_rules, Some(body_json)) {
        return None;
    }
    Some(provider_request_body)
}

fn build_video_upstream_url(
    parts: &http::request::Parts,
    transport: &GatewayProviderTransportSnapshot,
    mapped_model: &str,
    family: LocalVideoCreateFamily,
) -> Option<String> {
    let custom_path = transport
        .endpoint
        .custom_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(path) = custom_path {
        let blocked_keys = match family {
            LocalVideoCreateFamily::OpenAi => &[][..],
            LocalVideoCreateFamily::Gemini => &["key"][..],
        };
        return build_passthrough_path_url(
            &transport.endpoint.base_url,
            path,
            parts.uri.query(),
            blocked_keys,
        );
    }

    match family {
        LocalVideoCreateFamily::OpenAi => build_passthrough_path_url(
            &transport.endpoint.base_url,
            parts.uri.path(),
            parts.uri.query(),
            &[],
        ),
        LocalVideoCreateFamily::Gemini => build_gemini_video_predict_long_running_url(
            &transport.endpoint.base_url,
            mapped_model,
            parts.uri.query(),
        ),
    }
}

async fn materialize_local_video_create_candidate_attempts(
    state: &AppState,
    trace_id: &str,
    input: &LocalVideoCreateDecisionInput,
    candidates: Vec<SchedulerMinimalCandidateSelectionCandidate>,
    api_format: &str,
) -> Vec<LocalVideoCreateCandidateAttempt> {
    let candidates = prefer_local_tunnel_owner_candidates(state, candidates).await;
    let created_at_unix_secs = current_unix_secs();
    let mut attempts = Vec::with_capacity(candidates.len());

    for (candidate_index, candidate) in candidates.into_iter().enumerate() {
        let generated_candidate_id = Uuid::new_v4().to_string();
        let extra_data = json!({
            "provider_api_format": api_format,
            "client_api_format": api_format,
            "global_model_id": candidate.global_model_id.clone(),
            "global_model_name": candidate.global_model_name.clone(),
            "model_id": candidate.model_id.clone(),
            "selected_provider_model_name": candidate.selected_provider_model_name.clone(),
            "mapping_matched_model": candidate.mapping_matched_model.clone(),
            "provider_name": candidate.provider_name.clone(),
            "key_name": candidate.key_name.clone(),
        });

        let candidate_id = persist_available_local_candidate(
            state,
            trace_id,
            &input.auth_context.user_id,
            &input.auth_context.api_key_id,
            &candidate,
            candidate_index as u32,
            &generated_candidate_id,
            Some(extra_data),
            created_at_unix_secs,
            "gateway local video decision request candidate upsert failed",
        )
        .await;

        attempts.push(LocalVideoCreateCandidateAttempt {
            candidate,
            candidate_index: candidate_index as u32,
            candidate_id,
        });
    }

    attempts
}

async fn mark_skipped_local_video_candidate(
    state: &AppState,
    input: &LocalVideoCreateDecisionInput,
    trace_id: &str,
    candidate: &SchedulerMinimalCandidateSelectionCandidate,
    candidate_index: u32,
    candidate_id: &str,
    skip_reason: &'static str,
) {
    persist_skipped_local_candidate(
        state,
        trace_id,
        &input.auth_context.user_id,
        &input.auth_context.api_key_id,
        candidate,
        candidate_index,
        candidate_id,
        skip_reason,
        current_unix_secs(),
        "gateway local video decision failed to persist skipped candidate",
    )
    .await;
}

async fn mark_unused_local_video_candidates(
    state: &AppState,
    remaining: Vec<LocalSyncPlanAndReport>,
) {
    mark_unused_local_candidate_items(
        state,
        remaining,
        |item| &item.plan,
        |item| item.report_context.as_ref(),
    )
    .await;
}

fn extract_gemini_video_model_from_path(path: &str) -> Option<String> {
    let suffix = path.strip_prefix("/v1beta/models/")?;
    let model = suffix.split(':').next()?.trim();
    if model.is_empty() {
        return None;
    }
    Some(model.to_string())
}
