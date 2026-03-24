use std::sync::Arc;

use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidates::{
    InMemoryRequestCandidateRepository, RequestCandidateStatus, StoredRequestCandidate,
};
use aether_data::repository::provider_catalog::{
    InMemoryProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use aether_data::repository::shadow_results::{
    InMemoryShadowResultRepository, ShadowResultMatchStatus, ShadowResultReadRepository,
};
use aether_data::repository::usage::{InMemoryUsageReadRepository, StoredRequestUsageAudit};
use serde_json::Value;

use super::*;

#[tokio::test]
async fn gateway_records_shadow_result_for_ai_public_proxy_response() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/resolve",
            any(|_request: Request| async move {
                Json(json!({
                    "action": "proxy_public",
                    "route_class": "ai_public",
                    "route_family": "openai",
                    "route_kind": "chat",
                    "auth_endpoint_signature": "openai:chat",
                    "executor_candidate": false,
                    "public_path": "/v1/chat/completions"
                }))
            }),
        )
        .route(
            "/v1/chat/completions",
            any(|_request: Request| async move {
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from("{\"id\":\"chatcmpl-shadow\"}"))
                    .expect("response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }),
        );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway_state = AppState::new(upstream_url.clone(), Some(upstream_url))
        .expect("gateway state should build")
        .with_shadow_result_data_writer_for_tests(repository.clone());
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body("{\"model\":\"gpt-4.1\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTROL_ROUTE_CLASS_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("ai_public")
    );

    let response_trace_id = response
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("trace id should exist")
        .to_string();
    assert_eq!(
        response.text().await.expect("body should read"),
        "{\"id\":\"chatcmpl-shadow\"}"
    );

    for _ in 0..50 {
        if repository
            .list_recent(1)
            .await
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let stored = repository
        .list_recent(1)
        .await
        .expect("list should succeed")
        .into_iter()
        .next()
        .expect("stored result should exist");
    assert_eq!(stored.trace_id, response_trace_id);
    assert!(stored.request_id.is_none());
    assert_eq!(stored.route_family.as_deref(), Some("openai"));
    assert_eq!(stored.route_kind.as_deref(), Some("chat"));
    assert_eq!(stored.match_status, ShadowResultMatchStatus::Pending);
    assert_eq!(stored.status_code, Some(200));
    assert!(stored.rust_result_digest.is_some());
    assert!(stored.python_result_digest.is_none());

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_records_candidate_id_in_shadow_result_for_direct_executor_response() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());

    let upstream = Router::new().route(
        "/api/internal/gateway/plan-sync",
        any(|_request: Request| async move {
            Json(json!({
                "action": "executor_sync",
                "plan_kind": "openai_chat_sync",
                "plan": {
                    "request_id": "req-shadow-direct-123",
                    "candidate_id": "cand-shadow-direct-123",
                    "provider_name": "openai",
                    "provider_id": "provider-shadow-direct-123",
                    "endpoint_id": "endpoint-shadow-direct-123",
                    "key_id": "key-shadow-direct-123",
                    "method": "POST",
                    "url": "https://api.openai.example/v1/chat/completions",
                    "headers": {
                        "authorization": "Bearer upstream-key",
                        "content-type": "application/json"
                    },
                    "body": {
                        "json_body": {
                            "model": "gpt-5",
                            "messages": []
                        }
                    },
                    "stream": false,
                    "client_api_format": "openai:chat",
                    "provider_api_format": "openai:chat",
                    "model_name": "gpt-5"
                }
            }))
        }),
    );

    let executor = Router::new().route(
        "/v1/execute/sync",
        any(|_request: Request| async move {
            Json(json!({
                "request_id": "req-shadow-direct-123",
                "candidate_id": "cand-shadow-direct-123",
                "status_code": 200,
                "headers": {
                    "content-type": "application/json"
                },
                "body": {
                    "json_body": {
                        "id": "chatcmpl-shadow-direct-123",
                        "object": "chat.completion",
                        "model": "gpt-5",
                        "choices": []
                    }
                }
            }))
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (executor_url, executor_handle) = start_server(executor).await;
    let gateway_state =
        AppState::new_with_executor(upstream_url.clone(), Some(upstream_url), Some(executor_url))
            .expect("gateway state should build")
            .with_shadow_result_data_repository_for_tests(repository.clone());
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body("{\"model\":\"gpt-5\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTROL_CANDIDATE_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("cand-shadow-direct-123")
    );

    for _ in 0..50 {
        if repository
            .list_recent(1)
            .await
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let stored = repository
        .list_recent(1)
        .await
        .expect("list should succeed")
        .into_iter()
        .next()
        .expect("stored result should exist");
    assert_eq!(stored.request_id.as_deref(), Some("req-shadow-direct-123"));
    assert_eq!(
        stored.candidate_id.as_deref(),
        Some("cand-shadow-direct-123")
    );
    assert_eq!(stored.route_family.as_deref(), Some("openai"));
    assert_eq!(stored.route_kind.as_deref(), Some("chat"));
    assert_eq!(stored.match_status, ShadowResultMatchStatus::Pending);

    gateway_handle.abort();
    executor_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_request_id_header_for_direct_executor_response() {
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_auth_snapshot("api-key-1", "user-1"),
    )]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-1",
            "req-direct-audit-123",
            0,
            RequestCandidateStatus::Success,
            Some(101),
            Some(37),
            Some(200),
        ),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_request_usage("req-direct-audit-123"),
    ]));

    let upstream = Router::new().route(
        "/api/internal/gateway/plan-sync",
        any(|_request: Request| async move {
            Json(json!({
                "action": "executor_sync",
                "plan_kind": "openai_chat_sync",
                "plan": {
                    "request_id": "req-direct-audit-123",
                    "candidate_id": "cand-direct-audit-123",
                    "provider_name": "openai",
                    "provider_id": "provider-direct-audit-123",
                    "endpoint_id": "endpoint-direct-audit-123",
                    "key_id": "key-direct-audit-123",
                    "method": "POST",
                    "url": "https://api.openai.example/v1/chat/completions",
                    "headers": {
                        "authorization": "Bearer upstream-key",
                        "content-type": "application/json"
                    },
                    "body": {
                        "json_body": {
                            "model": "gpt-5",
                            "messages": []
                        }
                    },
                    "stream": false,
                    "client_api_format": "openai:chat",
                    "provider_api_format": "openai:chat",
                    "model_name": "gpt-5"
                }
            }))
        }),
    );

    let executor = Router::new().route(
        "/v1/execute/sync",
        any(|_request: Request| async move {
            Json(json!({
                "request_id": "req-direct-audit-123",
                "candidate_id": "cand-direct-audit-123",
                "status_code": 200,
                "headers": {
                    "content-type": "application/json"
                },
                "body": {
                    "json_body": {
                        "id": "chatcmpl-direct-audit-123",
                        "object": "chat.completion",
                        "model": "gpt-5",
                        "choices": []
                    }
                }
            }))
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (executor_url, executor_handle) = start_server(executor).await;
    let gateway_state =
        AppState::new_with_executor(upstream_url.clone(), Some(upstream_url), Some(executor_url))
            .expect("gateway state should build")
            .with_request_audit_data_readers_for_tests(
                auth_repository,
                request_candidates,
                provider_catalog,
                usage_repository,
            );
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body("{\"model\":\"gpt-5\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let request_id = response
        .headers()
        .get(CONTROL_REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("request id header should exist")
        .to_string();
    assert_eq!(request_id, "req-direct-audit-123");

    let audit_response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/request-audit/{request_id}?attempted_only=true"
        ))
        .send()
        .await
        .expect("request audit should succeed");

    assert_eq!(audit_response.status(), StatusCode::OK);
    let payload: Value = audit_response.json().await.expect("payload should parse");
    assert_eq!(payload["request_id"], "req-direct-audit-123");
    assert_eq!(payload["usage"]["provider_name"], "OpenAI");
    assert_eq!(payload["decision_trace"]["total_candidates"], 1);
    assert_eq!(payload["auth_snapshot"]["api_key_id"], "api-key-1");

    gateway_handle.abort();
    executor_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_recent_shadow_results_via_internal_audit_endpoint() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/resolve",
            any(|_request: Request| async move {
                Json(json!({
                    "action": "proxy_public",
                    "route_class": "ai_public",
                    "route_family": "openai",
                    "route_kind": "chat",
                    "auth_endpoint_signature": "openai:chat",
                    "executor_candidate": false,
                    "public_path": "/v1/chat/completions"
                }))
            }),
        )
        .route(
            "/v1/chat/completions",
            any(|_request: Request| async move {
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from("{\"id\":\"chatcmpl-shadow-read\"}"))
                    .expect("response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
            }),
        );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway_state = AppState::new(upstream_url.clone(), Some(upstream_url))
        .expect("gateway state should build")
        .with_shadow_result_data_repository_for_tests(repository.clone());
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let write_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body("{\"model\":\"gpt-4.1\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(write_response.status(), StatusCode::OK);

    for _ in 0..50 {
        if repository
            .list_recent(1)
            .await
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/shadow-results/recent?limit=5"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["limit_applied"], 5);
    assert_eq!(payload["counts"]["pending"], 1);
    assert_eq!(payload["counts"]["match"], 0);
    assert_eq!(
        payload["items"].as_array().map(|items| items.len()),
        Some(1)
    );
    assert!(payload["items"][0]["request_id"].is_null());
    assert_eq!(payload["items"][0]["route_family"], "openai");
    assert_eq!(payload["items"][0]["route_kind"], "chat");
    assert_eq!(payload["items"][0]["match_status"], "Pending");

    gateway_handle.abort();
    upstream_handle.abort();
}

fn sample_request_candidate(
    id: &str,
    request_id: &str,
    candidate_index: i32,
    status: RequestCandidateStatus,
    started_at_unix_secs: Option<i64>,
    latency_ms: Option<i32>,
    status_code: Option<i32>,
) -> StoredRequestCandidate {
    StoredRequestCandidate::new(
        id.to_string(),
        request_id.to_string(),
        Some("user-1".to_string()),
        Some("api-key-1".to_string()),
        Some("alice".to_string()),
        Some("default".to_string()),
        candidate_index,
        0,
        Some("provider-1".to_string()),
        Some("endpoint-1".to_string()),
        Some("provider-key-1".to_string()),
        status,
        None,
        false,
        status_code,
        None,
        None,
        latency_ms,
        Some(1),
        None,
        None,
        100 + i64::from(candidate_index),
        started_at_unix_secs,
        started_at_unix_secs.map(|value| value + 1),
    )
    .expect("candidate should build")
}

fn sample_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
    StoredAuthApiKeySnapshot::new(
        user_id.to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        Some(serde_json::json!(["openai"])),
        Some(serde_json::json!(["openai:chat"])),
        Some(serde_json::json!(["gpt-4.1"])),
        api_key_id.to_string(),
        Some("default".to_string()),
        true,
        false,
        false,
        Some(60),
        Some(5),
        Some(4_102_444_800),
        Some(serde_json::json!(["openai"])),
        Some(serde_json::json!(["openai:chat"])),
        Some(serde_json::json!(["gpt-4.1"])),
    )
    .expect("auth snapshot should build")
}

fn sample_provider_catalog_provider() -> StoredProviderCatalogProvider {
    StoredProviderCatalogProvider::new(
        "provider-1".to_string(),
        "OpenAI".to_string(),
        Some("https://openai.com".to_string()),
        "custom".to_string(),
    )
    .expect("provider should build")
}

fn sample_provider_catalog_endpoint() -> StoredProviderCatalogEndpoint {
    StoredProviderCatalogEndpoint::new(
        "endpoint-1".to_string(),
        "provider-1".to_string(),
        "openai:chat".to_string(),
        Some("openai".to_string()),
        Some("chat".to_string()),
        true,
    )
    .expect("endpoint should build")
}

fn sample_provider_catalog_key() -> StoredProviderCatalogKey {
    StoredProviderCatalogKey::new(
        "provider-key-1".to_string(),
        "provider-1".to_string(),
        "prod-key".to_string(),
        "api_key".to_string(),
        Some(serde_json::json!({"cache_1h": true})),
        true,
    )
    .expect("key should build")
}

fn sample_request_usage(request_id: &str) -> StoredRequestUsageAudit {
    StoredRequestUsageAudit::new(
        "usage-1".to_string(),
        request_id.to_string(),
        Some("user-1".to_string()),
        Some("api-key-1".to_string()),
        Some("alice".to_string()),
        Some("default".to_string()),
        "OpenAI".to_string(),
        "gpt-4.1".to_string(),
        Some("gpt-4.1-mini".to_string()),
        Some("provider-1".to_string()),
        Some("endpoint-1".to_string()),
        Some("provider-key-1".to_string()),
        Some("chat".to_string()),
        Some("openai:chat".to_string()),
        Some("openai".to_string()),
        Some("chat".to_string()),
        Some("openai:chat".to_string()),
        Some("openai".to_string()),
        Some("chat".to_string()),
        true,
        false,
        120,
        40,
        160,
        0.24,
        0.36,
        Some(200),
        None,
        None,
        Some(450),
        Some(120),
        "completed".to_string(),
        "settled".to_string(),
        100,
        101,
        Some(102),
    )
    .expect("usage should build")
}

#[tokio::test]
async fn gateway_exposes_request_usage_via_internal_audit_endpoint() {
    let repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_request_usage("req-usage-2"),
    ]));
    let gateway_state = AppState::new("http://127.0.0.1:18091", None)
        .expect("gateway state should build")
        .with_usage_data_reader_for_tests(repository);
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/request-usage/req-usage-2"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["request_id"], "req-usage-2");
    assert_eq!(payload["provider_name"], "OpenAI");
    assert_eq!(payload["api_format"], "openai:chat");
    assert_eq!(payload["total_tokens"], 160);
    assert_eq!(payload["total_cost_usd"], 0.24);
    assert_eq!(payload["status"], "completed");
    assert_eq!(payload["billing_status"], "settled");

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_request_audit_bundle_via_internal_audit_endpoint() {
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_auth_snapshot("api-key-1", "user-1"),
    )]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-1",
            "req-audit-1",
            0,
            RequestCandidateStatus::Success,
            Some(101),
            Some(37),
            Some(200),
        ),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_request_usage("req-audit-1"),
    ]));
    let gateway_state = AppState::new("http://127.0.0.1:18092", None)
        .expect("gateway state should build")
        .with_request_audit_data_readers_for_tests(
            auth_repository,
            request_candidates,
            provider_catalog,
            usage_repository,
        );
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/request-audit/req-audit-1?attempted_only=true"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["request_id"], "req-audit-1");
    assert_eq!(payload["usage"]["provider_name"], "OpenAI");
    assert_eq!(payload["usage"]["total_tokens"], 160);
    assert_eq!(payload["decision_trace"]["total_candidates"], 1);
    assert_eq!(
        payload["decision_trace"]["candidates"][0]["provider_key_name"],
        "prod-key"
    );
    assert_eq!(payload["auth_snapshot"]["api_key_id"], "api-key-1");
    assert_eq!(payload["auth_snapshot"]["currently_usable"], true);

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_request_candidate_trace_via_internal_audit_endpoint() {
    let repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-1",
            "req-trace-1",
            0,
            RequestCandidateStatus::Pending,
            None,
            None,
            None,
        ),
        sample_request_candidate(
            "cand-2",
            "req-trace-1",
            1,
            RequestCandidateStatus::Failed,
            Some(101),
            Some(37),
            Some(502),
        ),
    ]));

    let gateway_state = AppState::new("http://127.0.0.1:19081", None)
        .expect("gateway state should build")
        .with_request_candidate_data_reader_for_tests(repository);
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/request-candidates/req-trace-1?attempted_only=true"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["request_id"], "req-trace-1");
    assert_eq!(payload["total_candidates"], 1);
    assert_eq!(payload["final_status"], "failed");
    assert_eq!(payload["total_latency_ms"], 37);
    assert_eq!(
        payload["candidates"].as_array().map(|items| items.len()),
        Some(1)
    );
    assert_eq!(payload["candidates"][0]["id"], "cand-2");
    assert_eq!(payload["candidates"][0]["status"], "failed");

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_decision_trace_via_internal_audit_endpoint() {
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-1",
            "req-trace-2",
            0,
            RequestCandidateStatus::Failed,
            Some(101),
            Some(37),
            Some(502),
        ),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));

    let gateway_state = AppState::new("http://127.0.0.1:19083", None)
        .expect("gateway state should build")
        .with_decision_trace_data_readers_for_tests(request_candidates, provider_catalog);
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/decision-trace/req-trace-2?attempted_only=true"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["request_id"], "req-trace-2");
    assert_eq!(payload["total_candidates"], 1);
    assert_eq!(payload["candidates"][0]["provider_name"], "OpenAI");
    assert_eq!(
        payload["candidates"][0]["provider_website"],
        "https://openai.com"
    );
    assert_eq!(
        payload["candidates"][0]["endpoint_api_format"],
        "openai:chat"
    );
    assert_eq!(payload["candidates"][0]["provider_key_name"], "prod-key");
    assert_eq!(
        payload["candidates"][0]["provider_key_auth_type"],
        "api_key"
    );
    assert_eq!(
        payload["candidates"][0]["provider_key_capabilities"]["cache_1h"],
        true
    );

    gateway_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_auth_api_key_snapshot_via_internal_audit_endpoint() {
    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_auth_snapshot("key-1", "user-1"),
    )]));

    let gateway_state = AppState::new("http://127.0.0.1:19082", None)
        .expect("gateway state should build")
        .with_auth_api_key_data_reader_for_tests(repository);
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!(
            "{gateway_url}/_gateway/audit/auth/users/user-1/api-keys/key-1"
        ))
        .send()
        .await
        .expect("audit request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response.json().await.expect("payload should parse");
    assert_eq!(payload["user_id"], "user-1");
    assert_eq!(payload["api_key_id"], "key-1");
    assert_eq!(payload["username"], "alice");
    assert_eq!(payload["user_role"], "user");
    assert_eq!(payload["api_key_name"], "default");
    assert_eq!(payload["currently_usable"], true);
    assert_eq!(payload["user_allowed_providers"][0], "openai");
    assert_eq!(payload["api_key_allowed_api_formats"][0], "openai:chat");

    gateway_handle.abort();
}
