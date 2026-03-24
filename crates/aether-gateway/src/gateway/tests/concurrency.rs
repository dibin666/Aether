use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

#[tokio::test]
async fn gateway_rejects_second_in_flight_stream_request_with_distributed_overload() {
    let upstream_hits = Arc::new(AtomicUsize::new(0));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits = Arc::clone(&upstream_hits_clone);
            async move {
                upstream_hits.fetch_add(1, Ordering::SeqCst);
                let stream = async_stream::stream! {
                    yield Ok::<_, Infallible>(Bytes::from_static(b"chunk-1"));
                    futures_util::future::pending::<()>().await;
                };
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from_stream(stream))
                    .expect("response should build")
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let distributed_gate = aether_runtime::DistributedConcurrencyGate::new_in_memory(
        "gateway_requests_distributed",
        1,
    );
    let gateway_a = build_router_with_state(
        AppState::new(upstream_url.clone(), None)
            .expect("gateway state should build")
            .with_distributed_request_concurrency_gate(distributed_gate.clone()),
    );
    let gateway_b = build_router_with_state(
        AppState::new(upstream_url, None)
            .expect("gateway state should build")
            .with_distributed_request_concurrency_gate(distributed_gate),
    );
    let (gateway_a_url, gateway_a_handle) = start_server(gateway_a).await;
    let (gateway_b_url, gateway_b_handle) = start_server(gateway_b).await;

    let client = reqwest::Client::new();
    let first_response = client
        .get(format!("{gateway_a_url}/v1/messages"))
        .send()
        .await
        .expect("first request should succeed");

    wait_until(500, || upstream_hits.load(Ordering::SeqCst) == 1).await;

    let second_response = client
        .get(format!("{gateway_b_url}/v1/messages"))
        .send()
        .await
        .expect("second request should complete");

    assert_eq!(second_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        second_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(EXECUTION_PATH_DISTRIBUTED_OVERLOADED)
    );
    assert_eq!(
        second_response
            .json::<serde_json::Value>()
            .await
            .expect("json body should decode")["error"]["details"]["gate"],
        "gateway_requests_distributed"
    );
    assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

    drop(first_response);
    gateway_a_handle.abort();
    gateway_b_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_second_in_flight_stream_request_with_local_overload() {
    let upstream_hits = Arc::new(AtomicUsize::new(0));
    let upstream_hits_clone = Arc::clone(&upstream_hits);
    let upstream = Router::new().route(
        "/{*path}",
        any(move |_request: Request| {
            let upstream_hits = Arc::clone(&upstream_hits_clone);
            async move {
                upstream_hits.fetch_add(1, Ordering::SeqCst);
                let stream = async_stream::stream! {
                    yield Ok::<_, Infallible>(Bytes::from_static(b"chunk-1"));
                    futures_util::future::pending::<()>().await;
                };
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from_stream(stream))
                    .expect("response should build")
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let gateway = build_router_with_state(
        AppState::new(upstream_url, None)
            .expect("gateway state should build")
            .with_request_concurrency_limit(1),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let first_response = client
        .get(format!("{gateway_url}/v1/messages"))
        .send()
        .await
        .expect("first request should succeed");

    wait_until(500, || upstream_hits.load(Ordering::SeqCst) == 1).await;

    let second_response = client
        .get(format!("{gateway_url}/v1/messages"))
        .send()
        .await
        .expect("second request should complete");

    assert_eq!(second_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        second_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(EXECUTION_PATH_LOCAL_OVERLOADED)
    );
    assert_eq!(
        second_response
            .json::<serde_json::Value>()
            .await
            .expect("json body should decode")["error"]["type"],
        "overloaded"
    );
    assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

    drop(first_response);
    gateway_handle.abort();
    upstream_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_request_concurrency_metrics() {
    let gateway = build_router_with_state(
        AppState::new("http://127.0.0.1:1", None)
            .expect("gateway state should build")
            .with_request_concurrency_limit(3)
            .with_distributed_request_concurrency_gate(
                aether_runtime::DistributedConcurrencyGate::new_in_memory(
                    "gateway_requests_distributed",
                    5,
                ),
            ),
    );
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .get(format!("{gateway_url}/_gateway/metrics"))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let body = response.text().await.expect("body should read");
    assert!(body.contains("service_up{service=\"aether-gateway\"} 1"));
    assert!(body.contains("concurrency_in_flight{gate=\"gateway_requests\"} 0"));
    assert!(body.contains("concurrency_available_permits{gate=\"gateway_requests\"} 3"));
    assert!(body.contains("concurrency_in_flight{gate=\"gateway_requests_distributed\"} 0"));
    assert!(body.contains("concurrency_available_permits{gate=\"gateway_requests_distributed\"} 5"));

    gateway_handle.abort();
}
