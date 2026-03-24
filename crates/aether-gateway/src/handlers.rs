use std::time::Instant;

use aether_runtime::{maybe_hold_axum_response_permit, AdmissionPermit};
use axum::body::{to_bytes, Body};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::Response;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::TryStreamExt;
use serde_json::json;
use tracing::info;

use crate::gateway::constants::*;
use crate::gateway::headers::{
    extract_or_generate_trace_id, header_value_str, is_json_request, should_skip_request_header,
};
use crate::gateway::{
    build_client_response, build_local_auth_rejection_response, build_local_overloaded_response,
    maybe_execute_via_control, maybe_execute_via_executor_stream, maybe_execute_via_executor_sync,
    record_shadow_result_non_blocking, resolve_control_route, trusted_auth_local_rejection,
    AppState, GatewayControlDecision, GatewayError,
};

pub(crate) async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let request_concurrency = state.request_concurrency_snapshot().map(|snapshot| {
        json!({
            "limit": snapshot.limit,
            "in_flight": snapshot.in_flight,
            "available_permits": snapshot.available_permits,
            "high_watermark": snapshot.high_watermark,
            "rejected": snapshot.rejected,
        })
    });
    let distributed_request_concurrency = state
        .distributed_request_concurrency_snapshot()
        .await
        .ok()
        .flatten()
        .map(|snapshot| {
            json!({
                "limit": snapshot.limit,
                "in_flight": snapshot.in_flight,
                "available_permits": snapshot.available_permits,
                "high_watermark": snapshot.high_watermark,
                "rejected": snapshot.rejected,
            })
        });
    Json(json!({
        "status": "ok",
        "component": "aether-gateway",
        "control_api_enabled": state.control_base_url.is_some(),
        "request_concurrency": request_concurrency,
        "distributed_request_concurrency": distributed_request_concurrency,
    }))
}

pub(crate) async fn proxy_request(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<std::net::SocketAddr>,
    request: Request,
) -> Result<Response<Body>, GatewayError> {
    let started_at = Instant::now();
    let mut request_permit = match state.try_acquire_request_permit().await {
        Ok(permit) => permit,
        Err(crate::gateway::RequestAdmissionError::Local(
            aether_runtime::ConcurrencyError::Saturated { gate, limit },
        )) => {
            let trace_id = extract_or_generate_trace_id(request.headers());
            let response = build_local_overloaded_response(&trace_id, None, gate, limit)?;
            return Ok(finalize_gateway_response(
                &state,
                response,
                &trace_id,
                &remote_addr,
                request.method(),
                request
                    .uri()
                    .path_and_query()
                    .map(|value| value.as_str())
                    .unwrap_or("/"),
                None,
                EXECUTION_PATH_LOCAL_OVERLOADED,
                &started_at,
                None,
            ));
        }
        Err(crate::gateway::RequestAdmissionError::Local(
            aether_runtime::ConcurrencyError::Closed { gate },
        )) => {
            return Err(GatewayError::Internal(format!(
                "gateway request concurrency gate {gate} is closed"
            )));
        }
        Err(crate::gateway::RequestAdmissionError::Distributed(
            aether_runtime::DistributedConcurrencyError::Saturated { gate, limit },
        ))
        | Err(crate::gateway::RequestAdmissionError::Distributed(
            aether_runtime::DistributedConcurrencyError::Unavailable { gate, limit, .. },
        )) => {
            let trace_id = extract_or_generate_trace_id(request.headers());
            let response = build_local_overloaded_response(&trace_id, None, gate, limit)?;
            return Ok(finalize_gateway_response(
                &state,
                response,
                &trace_id,
                &remote_addr,
                request.method(),
                request
                    .uri()
                    .path_and_query()
                    .map(|value| value.as_str())
                    .unwrap_or("/"),
                None,
                EXECUTION_PATH_DISTRIBUTED_OVERLOADED,
                &started_at,
                None,
            ));
        }
        Err(crate::gateway::RequestAdmissionError::Distributed(
            aether_runtime::DistributedConcurrencyError::InvalidConfiguration(message),
        )) => return Err(GatewayError::Internal(message)),
    };
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");

    let host_header = header_value_str(&parts.headers, http::header::HOST.as_str());
    let trace_id = extract_or_generate_trace_id(&parts.headers);
    let control_decision =
        resolve_control_route(&state, &method, &parts.uri, &parts.headers, &trace_id).await?;
    if let Some(rejection) = trusted_auth_local_rejection(control_decision.as_ref(), &parts.headers)
    {
        let response =
            build_local_auth_rejection_response(&trace_id, control_decision.as_ref(), &rejection)?;
        return Ok(finalize_gateway_response(
            &state,
            response,
            &trace_id,
            &remote_addr,
            &method,
            path_and_query,
            control_decision.as_ref(),
            EXECUTION_PATH_LOCAL_AUTH_DENIED,
            &started_at,
            request_permit.take(),
        ));
    }
    let upstream_path_and_query = control_decision
        .as_ref()
        .map(|decision| decision.proxy_path_and_query())
        .unwrap_or_else(|| path_and_query.to_string());
    let target_url = format!("{}{}", state.upstream_base_url, upstream_path_and_query);
    let should_try_control_execute = control_decision
        .as_ref()
        .map(|decision| {
            decision.executor_candidate && decision.route_class.as_deref() == Some("ai_public")
        })
        .unwrap_or(false);

    let mut upstream_request = state.client.request(method.clone(), &target_url);
    for (name, value) in &parts.headers {
        if should_skip_request_header(name.as_str()) {
            continue;
        }
        upstream_request = upstream_request.header(name, value);
    }

    if let Some(host) = host_header.as_deref() {
        if !parts.headers.contains_key(FORWARDED_HOST_HEADER) {
            upstream_request = upstream_request.header(FORWARDED_HOST_HEADER, host);
        }
    }

    if !parts.headers.contains_key(FORWARDED_FOR_HEADER) {
        upstream_request =
            upstream_request.header(FORWARDED_FOR_HEADER, remote_addr.ip().to_string());
    }

    if !parts.headers.contains_key(FORWARDED_PROTO_HEADER) {
        upstream_request = upstream_request.header(FORWARDED_PROTO_HEADER, "http");
    }

    if !parts.headers.contains_key(TRACE_ID_HEADER) {
        upstream_request = upstream_request.header(TRACE_ID_HEADER, &trace_id);
    }

    if let Some(decision) = control_decision.as_ref() {
        upstream_request = upstream_request
            .header(
                CONTROL_ROUTE_CLASS_HEADER,
                decision.route_class.as_deref().unwrap_or("passthrough"),
            )
            .header(
                CONTROL_EXECUTOR_HEADER,
                if decision.executor_candidate {
                    "true"
                } else {
                    "false"
                },
            );
        if let Some(route_family) = decision.route_family.as_deref() {
            upstream_request = upstream_request.header(CONTROL_ROUTE_FAMILY_HEADER, route_family);
        }
        if let Some(route_kind) = decision.route_kind.as_deref() {
            upstream_request = upstream_request.header(CONTROL_ROUTE_KIND_HEADER, route_kind);
        }
        if let Some(endpoint_signature) = decision.auth_endpoint_signature.as_deref() {
            upstream_request =
                upstream_request.header(CONTROL_ENDPOINT_SIGNATURE_HEADER, endpoint_signature);
        }
        if let Some(auth_context) = decision.auth_context.as_ref() {
            upstream_request = upstream_request
                .header(TRUSTED_AUTH_USER_ID_HEADER, &auth_context.user_id)
                .header(TRUSTED_AUTH_API_KEY_ID_HEADER, &auth_context.api_key_id)
                .header(
                    TRUSTED_AUTH_ACCESS_ALLOWED_HEADER,
                    if auth_context.access_allowed {
                        "true"
                    } else {
                        "false"
                    },
                );
            if let Some(balance_remaining) = auth_context.balance_remaining {
                upstream_request = upstream_request
                    .header(TRUSTED_AUTH_BALANCE_HEADER, balance_remaining.to_string());
            }
        }
    }

    upstream_request = upstream_request.header(GATEWAY_HEADER, "rust-phase3b");

    let allow_control_execute_fallback = should_try_control_execute
        && (state.executor_base_url.is_none()
            || header_value_str(&parts.headers, CONTROL_EXECUTE_FALLBACK_HEADER)
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false));

    let upstream_response = if should_try_control_execute {
        let buffered_body = to_bytes(body, usize::MAX)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        let stream_request = request_wants_stream(&parts, &buffered_body);
        if stream_request {
            if let Some(executor_response) = maybe_execute_via_executor_stream(
                &state,
                &parts,
                &buffered_body,
                &trace_id,
                control_decision.as_ref(),
            )
            .await?
            {
                return Ok(finalize_gateway_response(
                    &state,
                    executor_response,
                    &trace_id,
                    &remote_addr,
                    &method,
                    path_and_query,
                    control_decision.as_ref(),
                    EXECUTION_PATH_EXECUTOR_STREAM,
                    &started_at,
                    request_permit.take(),
                ));
            }
        }
        if let Some(executor_response) = maybe_execute_via_executor_sync(
            &state,
            &parts,
            &buffered_body,
            &trace_id,
            control_decision.as_ref(),
        )
        .await?
        {
            return Ok(finalize_gateway_response(
                &state,
                executor_response,
                &trace_id,
                &remote_addr,
                &method,
                path_and_query,
                control_decision.as_ref(),
                EXECUTION_PATH_EXECUTOR_SYNC,
                &started_at,
                request_permit.take(),
            ));
        }
        if parts.method != http::Method::POST {
            if let Some(executor_response) = maybe_execute_via_executor_stream(
                &state,
                &parts,
                &buffered_body,
                &trace_id,
                control_decision.as_ref(),
            )
            .await?
            {
                return Ok(finalize_gateway_response(
                    &state,
                    executor_response,
                    &trace_id,
                    &remote_addr,
                    &method,
                    path_and_query,
                    control_decision.as_ref(),
                    EXECUTION_PATH_EXECUTOR_STREAM,
                    &started_at,
                    request_permit.take(),
                ));
            }
        }
        if allow_control_execute_fallback {
            if let Some(control_response) = maybe_execute_via_control(
                &state,
                &parts,
                buffered_body.clone(),
                &trace_id,
                control_decision.as_ref(),
            )
            .await?
            {
                return Ok(finalize_gateway_response(
                    &state,
                    control_response,
                    &trace_id,
                    &remote_addr,
                    &method,
                    path_and_query,
                    control_decision.as_ref(),
                    if stream_request {
                        EXECUTION_PATH_CONTROL_EXECUTE_STREAM
                    } else {
                        EXECUTION_PATH_CONTROL_EXECUTE_SYNC
                    },
                    &started_at,
                    request_permit.take(),
                ));
            }
        }
        upstream_request = upstream_request.header(
            EXECUTION_PATH_HEADER,
            EXECUTION_PATH_PUBLIC_PROXY_AFTER_EXECUTOR_MISS,
        );
        upstream_request
            .body(buffered_body)
            .send()
            .await
            .map_err(|err| GatewayError::UpstreamUnavailable {
                trace_id: trace_id.clone(),
                message: err.to_string(),
            })?
    } else {
        upstream_request = upstream_request.header(
            EXECUTION_PATH_HEADER,
            EXECUTION_PATH_PUBLIC_PROXY_PASSTHROUGH,
        );
        let request_body_stream = body
            .into_data_stream()
            .map_err(|err| std::io::Error::other(err.to_string()));
        upstream_request
            .body(reqwest::Body::wrap_stream(request_body_stream))
            .send()
            .await
            .map_err(|err| GatewayError::UpstreamUnavailable {
                trace_id: trace_id.clone(),
                message: err.to_string(),
            })?
    };

    let response = build_client_response(upstream_response, &trace_id, control_decision.as_ref())?;
    Ok(finalize_gateway_response(
        &state,
        response,
        &trace_id,
        &remote_addr,
        &method,
        path_and_query,
        control_decision.as_ref(),
        if should_try_control_execute {
            EXECUTION_PATH_PUBLIC_PROXY_AFTER_EXECUTOR_MISS
        } else {
            EXECUTION_PATH_PUBLIC_PROXY_PASSTHROUGH
        },
        &started_at,
        request_permit.take(),
    ))
}

fn request_wants_stream(parts: &http::request::Parts, body: &axum::body::Bytes) -> bool {
    if parts.uri.path().contains(":streamGenerateContent") {
        return true;
    }
    if !is_json_request(&parts.headers) || body.is_empty() {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("stream").and_then(|stream| stream.as_bool()))
        .unwrap_or(false)
}

fn finalize_gateway_response(
    state: &AppState,
    mut response: Response<Body>,
    trace_id: &str,
    remote_addr: &std::net::SocketAddr,
    method: &http::Method,
    path_and_query: &str,
    control_decision: Option<&GatewayControlDecision>,
    execution_path: &'static str,
    started_at: &Instant,
    request_permit: Option<AdmissionPermit>,
) -> Response<Body> {
    response.headers_mut().insert(
        HeaderName::from_static(EXECUTION_PATH_HEADER),
        HeaderValue::from_static(execution_path),
    );

    let elapsed_ms = started_at.elapsed().as_millis() as u64;
    info!(
        trace_id = %trace_id,
        remote_addr = %remote_addr,
        method = %method,
        path = %path_and_query,
        route_class = control_decision
            .and_then(|decision| decision.route_class.as_deref())
            .unwrap_or("passthrough"),
        execution_path,
        status = response.status().as_u16(),
        elapsed_ms,
        "gateway completed request"
    );

    record_shadow_result_non_blocking(
        state.clone(),
        trace_id,
        method,
        path_and_query,
        control_decision,
        execution_path,
        &response,
    );

    maybe_hold_axum_response_permit(response, request_permit)
}
