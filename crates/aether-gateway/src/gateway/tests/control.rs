use super::*;
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};

fn sample_currently_usable_auth_snapshot(
    api_key_id: &str,
    user_id: &str,
) -> StoredAuthApiKeySnapshot {
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
        Some(serde_json::json!(["gpt-5"])),
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
        Some(serde_json::json!(["gpt-5"])),
    )
    .expect("auth snapshot should build")
}

fn sample_expired_auth_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
    let mut snapshot = sample_currently_usable_auth_snapshot(api_key_id, user_id);
    snapshot.api_key_expires_at_unix_secs = Some(1);
    snapshot
}

#[tokio::test]
async fn gateway_consults_control_api_for_ai_routes_and_propagates_decision_headers() {
    #[derive(Debug, Clone)]
    struct SeenControlRequest {
        auth_endpoint_signature: String,
        query_string: String,
        trace_id: String,
    }

    #[derive(Debug, Clone)]
    struct SeenPublicRequest {
        control_route_class: String,
        control_route_family: String,
        control_route_kind: String,
        control_executor_candidate: String,
        control_endpoint_signature: String,
        trusted_user_id: String,
        trusted_api_key_id: String,
        trusted_balance_remaining: String,
        trusted_access_allowed: String,
        trace_id: String,
    }

    let seen_control = Arc::new(Mutex::new(None::<SeenControlRequest>));
    let seen_control_clone = Arc::clone(&seen_control);
    let seen_public = Arc::new(Mutex::new(None::<SeenPublicRequest>));
    let seen_public_clone = Arc::clone(&seen_public);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/auth-context",
            any(move |request: Request| {
                let seen_control_inner = Arc::clone(&seen_control_clone);
                async move {
                    let (parts, body) = request.into_parts();
                    let raw_body = to_bytes(body, usize::MAX).await.expect("body should read");
                    let payload: serde_json::Value =
                        serde_json::from_slice(&raw_body).expect("control payload should parse");
                    *seen_control_inner.lock().expect("mutex should lock") =
                        Some(SeenControlRequest {
                            auth_endpoint_signature: payload
                                .get("auth_endpoint_signature")
                                .and_then(|value| value.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            query_string: payload
                                .get("query_string")
                                .and_then(|value| value.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            trace_id: parts
                                .headers
                                .get(TRACE_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                        });
                    Json(json!({
                        "auth_context": {
                            "user_id": "user-123",
                            "api_key_id": "key-123",
                            "balance_remaining": 42.5,
                            "access_allowed": true
                        }
                    }))
                }
            }),
        )
        .route(
            "/v1/chat/completions",
            any(move |request: Request| {
                let seen_public_inner = Arc::clone(&seen_public_clone);
                async move {
                    *seen_public_inner.lock().expect("mutex should lock") =
                        Some(SeenPublicRequest {
                            control_route_class: request
                                .headers()
                                .get(CONTROL_ROUTE_CLASS_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            control_route_family: request
                                .headers()
                                .get(CONTROL_ROUTE_FAMILY_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            control_route_kind: request
                                .headers()
                                .get(CONTROL_ROUTE_KIND_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            control_executor_candidate: request
                                .headers()
                                .get(CONTROL_EXECUTOR_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            control_endpoint_signature: request
                                .headers()
                                .get(CONTROL_ENDPOINT_SIGNATURE_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_user_id: request
                                .headers()
                                .get(TRUSTED_AUTH_USER_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_api_key_id: request
                                .headers()
                                .get(TRUSTED_AUTH_API_KEY_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_balance_remaining: request
                                .headers()
                                .get(TRUSTED_AUTH_BALANCE_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_access_allowed: request
                                .headers()
                                .get(TRUSTED_AUTH_ACCESS_ALLOWED_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trace_id: request
                                .headers()
                                .get(TRACE_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                        });
                    (
                        StatusCode::OK,
                        [(GATEWAY_HEADER, "python-upstream")],
                        Body::from("proxied"),
                    )
                }
            }),
        );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_control(upstream_url.clone(), Some(upstream_url))
        .expect("gateway should build");
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions?stream=true"))
        .header(TRACE_ID_HEADER, "trace-control-123")
        .body("{\"hello\":\"world\"}")
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
    assert_eq!(
        response
            .headers()
            .get(CONTROL_EXECUTOR_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );

    let seen_control_request = seen_control
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("control request should be captured");
    assert_eq!(seen_control_request.auth_endpoint_signature, "openai:chat");
    assert_eq!(seen_control_request.query_string, "stream=true");
    assert_eq!(seen_control_request.trace_id, "trace-control-123");

    let seen_public_request = seen_public
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("public request should be captured");
    assert_eq!(seen_public_request.control_route_class, "ai_public");
    assert_eq!(seen_public_request.control_route_family, "openai");
    assert_eq!(seen_public_request.control_route_kind, "chat");
    assert_eq!(seen_public_request.control_executor_candidate, "true");
    assert_eq!(
        seen_public_request.control_endpoint_signature,
        "openai:chat"
    );
    assert_eq!(seen_public_request.trusted_user_id, "user-123");
    assert_eq!(seen_public_request.trusted_api_key_id, "key-123");
    assert_eq!(seen_public_request.trusted_balance_remaining, "42.5");
    assert_eq!(seen_public_request.trusted_access_allowed, "true");
    assert_eq!(seen_public_request.trace_id, "trace-control-123");

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_uses_data_backed_trusted_auth_context_without_calling_control_auth_endpoint() {
    #[derive(Debug, Clone)]
    struct SeenPublicRequest {
        trusted_user_id: String,
        trusted_api_key_id: String,
        trusted_balance_remaining: String,
        trusted_access_allowed: String,
    }

    let auth_context_hits = Arc::new(Mutex::new(0usize));
    let auth_context_hits_clone = Arc::clone(&auth_context_hits);
    let seen_public = Arc::new(Mutex::new(None::<SeenPublicRequest>));
    let seen_public_clone = Arc::clone(&seen_public);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/auth-context",
            any(move |_request: Request| {
                let auth_context_hits_inner = Arc::clone(&auth_context_hits_clone);
                async move {
                    *auth_context_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({
                        "auth_context": {
                            "user_id": "user-from-control",
                            "api_key_id": "key-from-control",
                            "balance_remaining": 99.0,
                            "access_allowed": true
                        }
                    }))
                }
            }),
        )
        .route(
            "/v1/chat/completions",
            any(move |request: Request| {
                let seen_public_inner = Arc::clone(&seen_public_clone);
                async move {
                    *seen_public_inner.lock().expect("mutex should lock") =
                        Some(SeenPublicRequest {
                            trusted_user_id: request
                                .headers()
                                .get(TRUSTED_AUTH_USER_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_api_key_id: request
                                .headers()
                                .get(TRUSTED_AUTH_API_KEY_ID_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_balance_remaining: request
                                .headers()
                                .get(TRUSTED_AUTH_BALANCE_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                            trusted_access_allowed: request
                                .headers()
                                .get(TRUSTED_AUTH_ACCESS_ALLOWED_HEADER)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                        });
                    (
                        StatusCode::OK,
                        [(GATEWAY_HEADER, "python-upstream")],
                        Body::from("proxied"),
                    )
                }
            }),
        );

    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_currently_usable_auth_snapshot("key-123", "user-123"),
    )]));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new(upstream_url.clone(), Some(upstream_url))
            .expect("gateway state should build")
            .with_auth_api_key_data_reader_for_tests(repository),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(TRACE_ID_HEADER, "trace-control-data-auth-1")
        .header(TRUSTED_AUTH_USER_ID_HEADER, "user-123")
        .header(TRUSTED_AUTH_API_KEY_ID_HEADER, "key-123")
        .header(TRUSTED_AUTH_BALANCE_HEADER, "42.5")
        .body("{\"hello\":\"world\"}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(*auth_context_hits.lock().expect("mutex should lock"), 0);

    let seen_public_request = seen_public
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("public request should be captured");
    assert_eq!(seen_public_request.trusted_user_id, "user-123");
    assert_eq!(seen_public_request.trusted_api_key_id, "key-123");
    assert_eq!(seen_public_request.trusted_balance_remaining, "42.5");
    assert_eq!(seen_public_request.trusted_access_allowed, "true");

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_locally_denies_explicit_trusted_balance_failure_without_hitting_control_or_upstream(
) {
    let auth_context_hits = Arc::new(Mutex::new(0usize));
    let auth_context_hits_clone = Arc::clone(&auth_context_hits);
    let public_hits = Arc::new(Mutex::new(0usize));
    let public_hits_clone = Arc::clone(&public_hits);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/auth-context",
            any(move |_request: Request| {
                let auth_context_hits_inner = Arc::clone(&auth_context_hits_clone);
                async move {
                    *auth_context_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({
                        "auth_context": {
                            "user_id": "user-from-control",
                            "api_key_id": "key-from-control",
                            "balance_remaining": 99.0,
                            "access_allowed": true
                        }
                    }))
                }
            }),
        )
        .route(
            "/v1/chat/completions",
            any(move |_request: Request| {
                let public_hits_inner = Arc::clone(&public_hits_clone);
                async move {
                    *public_hits_inner.lock().expect("mutex should lock") += 1;
                    (StatusCode::OK, Body::from("unexpected upstream hit"))
                }
            }),
        );

    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_currently_usable_auth_snapshot("key-123", "user-123"),
    )]));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new(upstream_url.clone(), Some(upstream_url))
            .expect("gateway state should build")
            .with_auth_api_key_data_reader_for_tests(repository),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(TRACE_ID_HEADER, "trace-control-balance-denied-1")
        .header(TRUSTED_AUTH_USER_ID_HEADER, "user-123")
        .header(TRUSTED_AUTH_API_KEY_ID_HEADER, "key-123")
        .header(TRUSTED_AUTH_BALANCE_HEADER, "0")
        .header(TRUSTED_AUTH_ACCESS_ALLOWED_HEADER, "false")
        .body("{\"model\":\"gpt-5\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(EXECUTION_PATH_LOCAL_AUTH_DENIED)
    );
    assert_eq!(
        response
            .headers()
            .get(CONTROL_ROUTE_CLASS_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("ai_public")
    );
    let payload: serde_json::Value = response.json().await.expect("response json should parse");
    assert_eq!(payload["error"]["type"], "balance_exceeded");
    assert_eq!(payload["error"]["message"], "余额不足（剩余: $0.00）");
    assert_eq!(payload["error"]["details"]["balance_type"], "USD");
    assert_eq!(payload["error"]["details"]["remaining"], 0.0);

    assert_eq!(*auth_context_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*public_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_locally_denies_invalid_trusted_snapshot_without_hitting_control_or_upstream() {
    let auth_context_hits = Arc::new(Mutex::new(0usize));
    let auth_context_hits_clone = Arc::clone(&auth_context_hits);
    let public_hits = Arc::new(Mutex::new(0usize));
    let public_hits_clone = Arc::clone(&public_hits);

    let upstream = Router::new()
        .route(
            "/api/internal/gateway/auth-context",
            any(move |_request: Request| {
                let auth_context_hits_inner = Arc::clone(&auth_context_hits_clone);
                async move {
                    *auth_context_hits_inner.lock().expect("mutex should lock") += 1;
                    Json(json!({
                        "auth_context": {
                            "user_id": "user-from-control",
                            "api_key_id": "key-from-control",
                            "access_allowed": true
                        }
                    }))
                }
            }),
        )
        .route(
            "/v1/chat/completions",
            any(move |_request: Request| {
                let public_hits_inner = Arc::clone(&public_hits_clone);
                async move {
                    *public_hits_inner.lock().expect("mutex should lock") += 1;
                    (StatusCode::OK, Body::from("unexpected upstream hit"))
                }
            }),
        );

    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_expired_auth_snapshot("key-123", "user-123"),
    )]));
    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new(upstream_url.clone(), Some(upstream_url))
            .expect("gateway state should build")
            .with_auth_api_key_data_reader_for_tests(repository),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/chat/completions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(TRACE_ID_HEADER, "trace-control-invalid-trusted-1")
        .header(TRUSTED_AUTH_USER_ID_HEADER, "user-123")
        .header(TRUSTED_AUTH_API_KEY_ID_HEADER, "key-123")
        .body("{\"model\":\"gpt-5\",\"messages\":[]}")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(EXECUTION_PATH_LOCAL_AUTH_DENIED)
    );
    let payload: serde_json::Value = response.json().await.expect("response json should parse");
    assert_eq!(payload["error"]["type"], "http_error");
    assert_eq!(payload["error"]["message"], "无效的API密钥");

    assert_eq!(*auth_context_hits.lock().expect("mutex should lock"), 0);
    assert_eq!(*public_hits.lock().expect("mutex should lock"), 0);

    gateway_handle.abort();
    upstream_handle.abort();
}
