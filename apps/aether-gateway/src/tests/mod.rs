pub(super) use std::convert::Infallible;
pub(super) use std::sync::{Arc, Mutex};

pub(super) use axum::body::{to_bytes, Body, Bytes};
pub(super) use axum::response::Response;
pub(super) use axum::routing::any;
pub(super) use axum::{extract::Request, Json, Router};
pub(super) use http::header::{HeaderName, HeaderValue};
pub(super) use http::StatusCode;
pub(super) use serde_json::json;

mod ai_execute;
mod architecture;
mod async_task;
mod audit;
mod concurrency;
mod control;
mod files;
mod frontdoor;
mod proxy;
mod usage;
mod video;

pub(super) use super::async_task::VideoTaskTruthSourceMode;
pub(super) use super::constants::*;
pub(super) use super::fallback_metrics::{GatewayFallbackMetricKind, GatewayFallbackReason};
pub(super) use super::rate_limit::FrontdoorUserRpmConfig;
pub(super) use super::router::{
    attach_static_frontend, build_router,
    build_router_with_state as build_production_router_with_state,
};
pub(super) use super::state::{AppState, FrontdoorCorsConfig};
pub(super) use super::usage::UsageRuntimeConfig;

#[derive(Clone)]
struct LegacyAdminAuthenticationState {
    app_state: AppState,
    access_token: std::sync::Arc<tokio::sync::OnceCell<String>>,
}

const LEGACY_ADMIN_TEST_DEVICE_ID: &str = "legacy-admin-test-device";

/// Translate legacy admin test fixtures to the real JWT/session authentication path.
///
/// The production router no longer accepts `x-aether-admin-*` identity headers.
/// A large set of existing endpoint tests still sends those headers, so this
/// test-only adapter keeps those fixtures useful without reintroducing a
/// production authentication bypass. Security regression tests use the
/// production router re-export directly.
pub(super) fn build_router_with_state(state: AppState) -> Router {
    let legacy_authentication_state = LegacyAdminAuthenticationState {
        app_state: state.clone(),
        access_token: std::sync::Arc::new(tokio::sync::OnceCell::const_new()),
    };
    build_production_router_with_state(state).layer(axum::middleware::from_fn_with_state(
        legacy_authentication_state,
        translate_legacy_admin_headers_middleware,
    ))
}

async fn translate_legacy_admin_headers_middleware(
    axum::extract::State(authentication_state): axum::extract::State<
        LegacyAdminAuthenticationState,
    >,
    mut request: Request,
    next: axum::middleware::Next,
) -> Response {
    let has_legacy_admin_headers = request
        .headers()
        .keys()
        .any(|name| name.as_str().starts_with("x-aether-admin-"));
    if has_legacy_admin_headers {
        if !request.headers().contains_key(http::header::AUTHORIZATION) {
            let app_state = authentication_state.app_state.clone();
            let access_token = authentication_state
                .access_token
                .get_or_init(|| async move {
                    control::helpers::issue_test_admin_access_token(
                        &app_state,
                        LEGACY_ADMIN_TEST_DEVICE_ID,
                    )
                    .await
                })
                .await;
            request.headers_mut().insert(
                http::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {access_token}"))
                    .expect("test authorization header should build"),
            );
        }
        if !request.headers().contains_key("x-client-device-id") {
            request.headers_mut().insert(
                HeaderName::from_static("x-client-device-id"),
                HeaderValue::from_static(LEGACY_ADMIN_TEST_DEVICE_ID),
            );
        }

        let headers_to_remove = request
            .headers()
            .keys()
            .filter(|name| name.as_str().starts_with("x-aether-admin-"))
            .cloned()
            .collect::<Vec<_>>();
        for name in headers_to_remove {
            request.headers_mut().remove(name);
        }
    }

    next.run(request).await
}

pub(super) async fn start_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = crate::test_support::bind_loopback_listener()
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("local addr should resolve");
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("server should run");
    });
    (format!("http://{addr}"), handle)
}

pub(super) async fn send_request(app: Router, mut request: Request) -> Response {
    use tower::ServiceExt;

    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))));
    app.oneshot(request)
        .await
        .expect("router request should complete")
}

pub(super) fn build_router_with_execution_runtime_override(
    execution_runtime_override_base_url: impl Into<String>,
) -> Router {
    let state = build_state_with_execution_runtime_override(execution_runtime_override_base_url);
    build_router_with_state(state)
}

pub(super) fn build_state_with_execution_runtime_override(
    execution_runtime_override_base_url: impl Into<String>,
) -> AppState {
    AppState::new()
        .expect("gateway should build")
        .with_execution_runtime_override_base_url(execution_runtime_override_base_url)
}

pub(super) async fn wait_until(timeout_ms: u64, mut predicate: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        if predicate() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "condition not met within {}ms",
            timeout_ms
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

pub(crate) fn strip_sse_keepalive_comments(body: &str) -> String {
    body.replace(": aether-keepalive\n\n", "")
}

pub(crate) async fn next_non_keepalive_chunk(response: &mut reqwest::Response) -> Bytes {
    loop {
        let chunk = response
            .chunk()
            .await
            .expect("chunk should read")
            .expect("chunk should exist");
        if chunk.as_ref() != b": aether-keepalive\n\n" {
            return chunk;
        }
    }
}
