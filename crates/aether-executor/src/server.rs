use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;

use aether_contracts::{
    ExecutionError, ExecutionErrorKind, ExecutionPhase, ExecutionPlan, ExecutionTelemetry,
    StreamFrame, StreamFramePayload, StreamFrameType,
};
use aether_runtime::{
    maybe_hold_axum_response_permit, prometheus_response, service_up_sample, AdmissionPermit,
    ConcurrencyError, ConcurrencyGate, ConcurrencySnapshot, DistributedConcurrencyError,
    DistributedConcurrencyGate, DistributedConcurrencySnapshot, MetricKind, MetricLabel,
    MetricSample,
};
use async_stream::stream;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::json;

use crate::{encode_frame, ExecutorServiceError, SyncExecutor};

#[derive(Debug, Clone, Default)]
pub struct AppState {
    executor: SyncExecutor,
    request_gate: Option<Arc<ConcurrencyGate>>,
    distributed_request_gate: Option<Arc<DistributedConcurrencyGate>>,
}

impl AppState {
    fn with_request_concurrency_limit(limit: Option<usize>) -> Self {
        Self {
            executor: SyncExecutor::new(),
            request_gate: limit
                .filter(|limit| *limit > 0)
                .map(|limit| Arc::new(ConcurrencyGate::new("executor_requests", limit))),
            distributed_request_gate: None,
        }
    }

    fn with_distributed_request_gate(mut self, gate: DistributedConcurrencyGate) -> Self {
        self.distributed_request_gate = Some(Arc::new(gate));
        self
    }

    fn request_concurrency_snapshot(&self) -> Option<ConcurrencySnapshot> {
        self.request_gate.as_ref().map(|gate| gate.snapshot())
    }

    async fn distributed_request_concurrency_snapshot(
        &self,
    ) -> Result<Option<DistributedConcurrencySnapshot>, DistributedConcurrencyError> {
        match self.distributed_request_gate.as_ref() {
            Some(gate) => gate.snapshot().await.map(Some),
            None => Ok(None),
        }
    }

    async fn metric_samples(&self) -> Vec<MetricSample> {
        let mut samples = vec![service_up_sample("aether-executor")];
        if let Some(snapshot) = self.request_concurrency_snapshot() {
            samples.extend(snapshot.to_metric_samples("executor_requests"));
        }
        if let Some(gate) = self.distributed_request_gate.as_ref() {
            match gate.snapshot().await {
                Ok(snapshot) => {
                    samples.extend(snapshot.to_metric_samples("executor_requests_distributed"));
                }
                Err(_) => samples.push(
                    MetricSample::new(
                        "concurrency_unavailable",
                        "Whether the distributed concurrency gate is currently unavailable.",
                        MetricKind::Gauge,
                        1,
                    )
                    .with_labels(vec![MetricLabel::new(
                        "gate",
                        "executor_requests_distributed",
                    )]),
                ),
            }
        }
        samples
    }

    async fn try_acquire_request_permit(
        &self,
    ) -> Result<Option<AdmissionPermit>, RequestAdmissionError> {
        let local = self
            .request_gate
            .as_ref()
            .map(|gate| gate.try_acquire())
            .transpose()
            .map_err(RequestAdmissionError::Local)?;
        let distributed = match self.distributed_request_gate.as_ref() {
            Some(gate) => Some(
                gate.try_acquire()
                    .await
                    .map_err(RequestAdmissionError::Distributed)?,
            ),
            None => None,
        };
        Ok(AdmissionPermit::from_parts(local, distributed))
    }
}

pub fn build_router() -> Router {
    build_router_with_request_concurrency_limit(None)
}

pub fn build_router_with_request_concurrency_limit(limit: Option<usize>) -> Router {
    build_router_with_request_gates(limit, None)
}

pub fn build_router_with_request_gates(
    limit: Option<usize>,
    distributed_gate: Option<DistributedConcurrencyGate>,
) -> Router {
    let state = match distributed_gate {
        Some(gate) => {
            AppState::with_request_concurrency_limit(limit).with_distributed_request_gate(gate)
        }
        None => AppState::with_request_concurrency_limit(limit),
    };
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/execute/sync", post(execute_sync))
        .route("/v1/execute/stream", post(execute_stream))
        .with_state(state)
}

pub async fn serve_tcp(
    bind: &str,
    max_in_flight_requests: Option<usize>,
    distributed_request_gate: Option<DistributedConcurrencyGate>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(
        listener,
        build_router_with_request_gates(max_in_flight_requests, distributed_request_gate),
    )
    .await?;
    Ok(())
}

pub async fn serve_unix(
    socket_path: &Path,
    max_in_flight_requests: Option<usize>,
    distributed_request_gate: Option<DistributedConcurrencyGate>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = tokio::net::UnixListener::bind(socket_path)?;
    axum::serve(
        listener,
        build_router_with_request_gates(max_in_flight_requests, distributed_request_gate),
    )
    .await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
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
        "component": "aether-executor",
        "request_concurrency": request_concurrency,
        "distributed_request_concurrency": distributed_request_concurrency,
    }))
}

async fn metrics(State(state): State<AppState>) -> Response {
    prometheus_response(&state.metric_samples().await)
}

async fn execute_sync(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, AppError> {
    let request_permit = acquire_request_permit(&state).await?;
    let plan = parse_request_json::<ExecutionPlan>(request).await?;
    let result = state.executor.execute_sync(plan).await.map_err(AppError)?;
    Ok(maybe_hold_axum_response_permit(
        Json(result).into_response(),
        request_permit,
    ))
}

async fn execute_stream(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, AppError> {
    let request_permit = acquire_request_permit(&state).await?;
    let plan = parse_request_json::<ExecutionPlan>(request).await?;
    let execution = state
        .executor
        .execute_stream(plan)
        .await
        .map_err(AppError)?;

    let status_code = execution.status_code;
    let response_headers = execution.headers.clone();
    let started_at = execution.started_at;
    let upstream_response = execution.response;

    let body_stream = stream! {
        let headers_frame = StreamFrame {
            frame_type: StreamFrameType::Headers,
            payload: StreamFramePayload::Headers {
                status_code,
                headers: response_headers,
            },
        };
        yield Ok::<Bytes, Infallible>(encode_frame(&headers_frame).expect("headers frame should encode"));

        let mut upstream_bytes = 0u64;
        let mut bytes_stream = upstream_response.bytes_stream();
        while let Some(item) = bytes_stream.next().await {
            match item {
                Ok(chunk) => {
                    upstream_bytes += chunk.len() as u64;
                    let frame = StreamFrame {
                        frame_type: StreamFrameType::Data,
                        payload: StreamFramePayload::Data {
                            chunk_b64: Some(base64::engine::general_purpose::STANDARD.encode(&chunk)),
                            text: None,
                        },
                    };
                    yield Ok::<Bytes, Infallible>(encode_frame(&frame).expect("data frame should encode"));
                }
                Err(err) => {
                    let frame = StreamFrame {
                        frame_type: StreamFrameType::Error,
                        payload: StreamFramePayload::Error {
                            error: ExecutionError {
                                kind: ExecutionErrorKind::Internal,
                                phase: ExecutionPhase::StreamRead,
                                message: err.to_string(),
                                upstream_status: Some(status_code),
                                retryable: false,
                                failover_recommended: false,
                            },
                        },
                    };
                    yield Ok::<Bytes, Infallible>(encode_frame(&frame).expect("error frame should encode"));
                    break;
                }
            }
        }

        let telemetry_frame = StreamFrame {
            frame_type: StreamFrameType::Telemetry,
            payload: StreamFramePayload::Telemetry {
                telemetry: ExecutionTelemetry {
                    ttfb_ms: None,
                    elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
                    upstream_bytes: Some(upstream_bytes),
                },
            },
        };
        yield Ok::<Bytes, Infallible>(encode_frame(&telemetry_frame).expect("telemetry frame should encode"));
        yield Ok::<Bytes, Infallible>(encode_frame(&StreamFrame::eof()).expect("eof frame should encode"));
    };

    let mut response = Response::new(Body::from_stream(body_stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-ndjson"),
    );
    Ok(maybe_hold_axum_response_permit(response, request_permit))
}

async fn acquire_request_permit(state: &AppState) -> Result<Option<AdmissionPermit>, AppError> {
    match state.try_acquire_request_permit().await {
        Ok(permit) => Ok(permit),
        Err(RequestAdmissionError::Local(ConcurrencyError::Saturated { gate, limit }))
        | Err(RequestAdmissionError::Distributed(DistributedConcurrencyError::Saturated {
            gate,
            limit,
        }))
        | Err(RequestAdmissionError::Distributed(DistributedConcurrencyError::Unavailable {
            gate,
            limit,
            ..
        })) => Err(AppError(ExecutorServiceError::Overloaded { gate, limit })),
        Err(RequestAdmissionError::Local(ConcurrencyError::Closed { gate })) => {
            Err(AppError(ExecutorServiceError::RequestRead(format!(
                "executor request concurrency gate {gate} is closed"
            ))))
        }
        Err(RequestAdmissionError::Distributed(
            DistributedConcurrencyError::InvalidConfiguration(message),
        )) => Err(AppError(ExecutorServiceError::RequestRead(message))),
    }
}

#[derive(Debug)]
enum RequestAdmissionError {
    Local(ConcurrencyError),
    Distributed(DistributedConcurrencyError),
}

async fn parse_request_json<T>(request: Request) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned,
{
    let body = to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|err| AppError(ExecutorServiceError::RequestRead(err.to_string())))?;
    serde_json::from_slice(&body)
        .map_err(|err| AppError(ExecutorServiceError::InvalidRequestJson(err)))
}

fn build_overloaded_response(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": {
                "type": "overloaded",
                "message": message,
            }
        })),
    )
        .into_response()
}

#[derive(Debug)]
struct AppError(ExecutorServiceError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = match self.0 {
            ExecutorServiceError::RequestRead(_) | ExecutorServiceError::InvalidRequestJson(_) => {
                StatusCode::BAD_REQUEST
            }
            ExecutorServiceError::Overloaded { .. } => {
                return build_overloaded_response(&self.0.to_string());
            }
            ExecutorServiceError::StreamUnsupported
            | ExecutorServiceError::RequestBodyRequired
            | ExecutorServiceError::BodyDecode(_)
            | ExecutorServiceError::UnsupportedContentEncoding(_)
            | ExecutorServiceError::ProxyUnsupported
            | ExecutorServiceError::TlsProfileUnsupported
            | ExecutorServiceError::DelegateUnsupported
            | ExecutorServiceError::InvalidMethod(_)
            | ExecutorServiceError::InvalidHeaderName(_)
            | ExecutorServiceError::InvalidHeaderValue(_)
            | ExecutorServiceError::InvalidProxy(_)
            | ExecutorServiceError::BodyEncode(_) => StatusCode::BAD_REQUEST,
            ExecutorServiceError::ClientBuild(_)
            | ExecutorServiceError::UpstreamRequest(_)
            | ExecutorServiceError::RelayError(_)
            | ExecutorServiceError::InvalidJson(_) => StatusCode::BAD_GATEWAY,
        };

        (
            status_code,
            Json(json!({
                "error": self.0.to_string(),
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_router_with_request_concurrency_limit, build_router_with_request_gates};
    use aether_contracts::{ExecutionPlan, ExecutionTimeouts, RequestBody};
    use axum::body::{Body, Bytes};
    use axum::response::Response;
    use axum::routing::any;
    use axum::{extract::Request, Router};
    use http::StatusCode;
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    async fn start_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should resolve");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });
        (format!("http://{addr}"), handle)
    }

    fn stream_plan(url: String) -> ExecutionPlan {
        ExecutionPlan {
            request_id: "req-1".into(),
            candidate_id: Some("cand-1".into()),
            provider_name: Some("openai".into()),
            provider_id: "prov-1".into(),
            endpoint_id: "ep-1".into(),
            key_id: "key-1".into(),
            method: "GET".into(),
            url,
            headers: std::collections::BTreeMap::new(),
            content_type: None,
            content_encoding: None,
            body: RequestBody {
                json_body: None,
                body_bytes_b64: None,
                body_ref: None,
            },
            stream: true,
            client_api_format: "openai:chat".into(),
            provider_api_format: "openai:chat".into(),
            model_name: Some("gpt-4.1".into()),
            proxy: None,
            tls_profile: None,
            timeouts: Some(ExecutionTimeouts {
                connect_ms: Some(5_000),
                total_ms: Some(30_000),
                ..ExecutionTimeouts::default()
            }),
        }
    }

    #[tokio::test]
    async fn executor_rejects_second_in_flight_stream_request_with_overload() {
        let upstream_hits = Arc::new(AtomicUsize::new(0));
        let upstream_hits_clone = Arc::clone(&upstream_hits);
        let upstream = Router::new().route(
            "/slow",
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
        let executor = build_router_with_request_concurrency_limit(Some(1));
        let (executor_url, executor_handle) = start_server(executor).await;

        let client = reqwest::Client::new();
        let first_response = client
            .post(format!("{executor_url}/v1/execute/stream"))
            .json(&stream_plan(format!("{upstream_url}/slow")))
            .send()
            .await
            .expect("first request should succeed");

        for _ in 0..50 {
            if upstream_hits.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

        let second_response = client
            .post(format!("{executor_url}/v1/execute/stream"))
            .json(&stream_plan(format!("{upstream_url}/slow")))
            .send()
            .await
            .expect("second request should complete");

        assert_eq!(second_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            second_response
                .json::<serde_json::Value>()
                .await
                .expect("json body should decode")["error"]["type"],
            "overloaded"
        );
        assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

        drop(first_response);
        executor_handle.abort();
        upstream_handle.abort();
    }

    #[tokio::test]
    async fn executor_rejects_second_in_flight_stream_request_with_distributed_overload() {
        let upstream_hits = Arc::new(AtomicUsize::new(0));
        let upstream_hits_clone = Arc::clone(&upstream_hits);
        let upstream = Router::new().route(
            "/slow",
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
            "executor_requests_distributed",
            1,
        );
        let executor_a = build_router_with_request_gates(None, Some(distributed_gate.clone()));
        let executor_b = build_router_with_request_gates(None, Some(distributed_gate));
        let (executor_a_url, executor_a_handle) = start_server(executor_a).await;
        let (executor_b_url, executor_b_handle) = start_server(executor_b).await;

        let client = reqwest::Client::new();
        let first_response = client
            .post(format!("{executor_a_url}/v1/execute/stream"))
            .json(&stream_plan(format!("{upstream_url}/slow")))
            .send()
            .await
            .expect("first request should succeed");

        for _ in 0..50 {
            if upstream_hits.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

        let second_response = client
            .post(format!("{executor_b_url}/v1/execute/stream"))
            .json(&stream_plan(format!("{upstream_url}/slow")))
            .send()
            .await
            .expect("second request should complete");

        assert_eq!(second_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            second_response
                .json::<serde_json::Value>()
                .await
                .expect("json body should decode")["error"]["type"],
            "overloaded"
        );
        assert_eq!(upstream_hits.load(Ordering::SeqCst), 1);

        drop(first_response);
        executor_a_handle.abort();
        executor_b_handle.abort();
        upstream_handle.abort();
    }

    #[tokio::test]
    async fn executor_exposes_request_concurrency_metrics() {
        let executor = build_router_with_request_gates(
            Some(4),
            Some(aether_runtime::DistributedConcurrencyGate::new_in_memory(
                "executor_requests_distributed",
                6,
            )),
        );
        let (executor_url, executor_handle) = start_server(executor).await;

        let response = reqwest::Client::new()
            .get(format!("{executor_url}/metrics"))
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
        assert!(body.contains("service_up{service=\"aether-executor\"} 1"));
        assert!(body.contains("concurrency_available_permits{gate=\"executor_requests\"} 4"));
        assert!(body
            .contains("concurrency_available_permits{gate=\"executor_requests_distributed\"} 6"));

        executor_handle.abort();
    }
}
