use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, json, start_server,
    strip_sse_keepalive_comments, to_bytes, Arc, Body, Mutex, Request, Router, StatusCode,
    EXECUTION_PATH_EXECUTION_RUNTIME_STREAM, EXECUTION_PATH_HEADER, TRACE_ID_HEADER,
};
use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data_contracts::repository::candidate_selection::StoredMinimalCandidateSelectionRow;
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use base64::Engine as _;
use sha2::{Digest, Sha256};

const TRANSCRIPTION_STREAM_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const CLIENT_MODEL: &str = "client-stream-transcribe";
const PROVIDER_MODEL: &str = "gpt-4o-transcribe-diarize";
const CLIENT_API_KEY: &str = "sk-client-transcription-stream";
const BOUNDARY: &str = "aether-transcription-stream-boundary";
const AUDIO_BYTES: &[u8] = b"\0\xffRIFFstream-audio\x80";
const UPSTREAM_SSE: &str = concat!(
    "event: transcript.text.delta\n",
    "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hel\"}\n\n",
    "event: transcript.text.done\n",
    "data: {\"type\":\"transcript.text.done\",\"text\":\"hello\",\"usage\":{\"type\":\"duration\",\"seconds\":2.5}}\n\n",
);

fn run_transcription_stream_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(TRANSCRIPTION_STREAM_TEST_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build")
                .block_on(make_future());
        })
        .expect("transcription stream test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn hash_api_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn auth_snapshot() -> StoredAuthApiKeySnapshot {
    StoredAuthApiKeySnapshot::new(
        "user-transcription-stream-1".to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        None,
        Some(json!(["openai:transcription"])),
        None,
        "api-key-transcription-stream-1".to_string(),
        Some("transcription-stream-client".to_string()),
        true,
        false,
        false,
        Some(60),
        Some(5),
        Some(4_102_444_800_i64),
        None,
        Some(json!(["openai:transcription"])),
        None,
    )
    .expect("auth snapshot should build")
}

fn candidate_row() -> StoredMinimalCandidateSelectionRow {
    StoredMinimalCandidateSelectionRow {
        provider_id: "provider-transcription-stream-1".to_string(),
        provider_name: "transcription-stream-provider".to_string(),
        provider_type: "custom".to_string(),
        provider_priority: 10,
        provider_is_active: true,
        endpoint_id: "endpoint-transcription-stream-1".to_string(),
        endpoint_api_format: "openai:transcription".to_string(),
        endpoint_api_family: Some("openai".to_string()),
        endpoint_kind: Some("transcription".to_string()),
        endpoint_is_active: true,
        key_id: "key-transcription-stream-1".to_string(),
        key_name: "stream-key".to_string(),
        key_auth_type: "api_key".to_string(),
        key_is_active: true,
        key_api_formats: Some(vec!["openai:transcription".to_string()]),
        key_allowed_models: None,
        key_capabilities: None,
        key_internal_priority: 1,
        key_global_priority_by_format: Some(json!({"openai:transcription": 1})),
        model_id: "model-transcription-stream-1".to_string(),
        global_model_id: "global-model-transcription-stream-1".to_string(),
        global_model_name: CLIENT_MODEL.to_string(),
        global_model_mappings: None,
        global_model_supports_streaming: Some(true),
        model_provider_model_name: PROVIDER_MODEL.to_string(),
        model_provider_model_mappings: None,
        model_supports_streaming: Some(true),
        model_is_active: true,
        model_is_available: true,
    }
}

fn provider() -> StoredProviderCatalogProvider {
    StoredProviderCatalogProvider::new(
        "provider-transcription-stream-1".to_string(),
        "transcription-stream-provider".to_string(),
        Some("https://stream-provider.example".to_string()),
        "custom".to_string(),
    )
    .expect("provider should build")
    .with_transport_fields(
        true,
        false,
        false,
        None,
        Some(1),
        None,
        Some(30.0),
        None,
        None,
    )
}

fn endpoint() -> StoredProviderCatalogEndpoint {
    StoredProviderCatalogEndpoint::new(
        "endpoint-transcription-stream-1".to_string(),
        "provider-transcription-stream-1".to_string(),
        "openai:transcription".to_string(),
        Some("openai".to_string()),
        Some("transcription".to_string()),
        true,
    )
    .expect("endpoint should build")
    .with_transport_fields(
        "https://stream-provider.example/v1".to_string(),
        None,
        None,
        Some(1),
        None,
        None,
        None,
        None,
    )
    .expect("endpoint transport should build")
}

fn key() -> StoredProviderCatalogKey {
    StoredProviderCatalogKey::new(
        "key-transcription-stream-1".to_string(),
        "provider-transcription-stream-1".to_string(),
        "stream-key".to_string(),
        "api_key".to_string(),
        None,
        true,
    )
    .expect("key should build")
    .with_transport_fields(
        Some(json!(["openai:transcription"])),
        encrypt_python_fernet_plaintext(
            DEVELOPMENT_ENCRYPTION_KEY,
            "sk-upstream-transcription-stream",
        )
        .expect("upstream key should encrypt"),
        None,
        None,
        Some(json!({"openai:transcription": 1})),
        None,
        None,
        None,
        None,
    )
    .expect("key transport should build")
}

fn multipart_body() -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in [
        ("model", CLIENT_MODEL),
        ("response_format", "diarized_json"),
        ("stream", "true"),
        ("chunking_strategy", "auto"),
    ] {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"meeting.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
    );
    body.extend_from_slice(AUDIO_BYTES);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

#[test]
fn gateway_streams_transcription_sse_verbatim_with_binary_plan_body() {
    run_transcription_stream_test(
        "gateway_streams_transcription_sse_verbatim_with_binary_plan_body",
        gateway_streams_transcription_sse_verbatim_with_binary_plan_body_impl,
    );
}

async fn gateway_streams_transcription_sse_verbatim_with_binary_plan_body_impl() {
    let seen_plan = Arc::new(Mutex::new(None::<serde_json::Value>));
    let seen_plan_clone = Arc::clone(&seen_plan);
    let execution_runtime = Router::new().route(
        "/v1/execute/stream",
        any(move |request: Request| {
            let seen_plan = Arc::clone(&seen_plan_clone);
            async move {
                let (_, body) = request.into_parts();
                let bytes = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("execution plan should parse");
                *seen_plan.lock().expect("mutex should lock") = Some(payload);
                let frames = format!(
                    "{{\"type\":\"headers\",\"payload\":{{\"kind\":\"headers\",\"status_code\":200,\"headers\":{{\"content-type\":\"text/event-stream\",\"x-transcription-upstream\":\"stream\"}}}}}}\n{{\"type\":\"data\",\"payload\":{{\"kind\":\"data\",\"text\":{}}}}}\n{{\"type\":\"telemetry\",\"payload\":{{\"kind\":\"telemetry\",\"telemetry\":{{\"elapsed_ms\":13}}}}}}\n{{\"type\":\"eof\",\"payload\":{{\"kind\":\"eof\"}}}}\n",
                    serde_json::to_string(UPSTREAM_SSE).expect("SSE should encode")
                );
                let mut response = http::Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(frames))
                    .expect("runtime response should build");
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("application/x-ndjson"),
                );
                response
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key(CLIENT_API_KEY)),
        auth_snapshot(),
    )]));
    let candidate_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            candidate_row(),
        ]));
    let catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![provider()],
        vec![endpoint()],
        vec![key()],
    ));
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let data_state = crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
        auth_repository,
        candidate_repository,
        catalog_repository,
        Arc::new(InMemoryRequestCandidateRepository::default()),
        DEVELOPMENT_ENCRYPTION_KEY,
    );
    let state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(data_state);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/audio/transcriptions"))
        .header(
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-transcription-stream-1")
        .body(multipart_body())
        .send()
        .await
        .expect("stream transcription request should complete");
    let status = response.status();
    let execution_path = response
        .headers()
        .get(EXECUTION_PATH_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let response_text = response.text().await.expect("stream body should read");
    assert_eq!(status, StatusCode::OK, "{response_text}");
    assert_eq!(
        execution_path.as_deref(),
        Some(EXECUTION_PATH_EXECUTION_RUNTIME_STREAM),
        "{response_text}"
    );
    assert_eq!(
        content_type.as_deref(),
        Some("text/event-stream"),
        "{response_text}"
    );
    assert_eq!(strip_sse_keepalive_comments(&response_text), UPSTREAM_SSE);

    let plan = seen_plan
        .lock()
        .expect("mutex should lock")
        .clone()
        .expect("stream plan should be captured");
    assert_eq!(
        plan["url"],
        "https://stream-provider.example/v1/audio/transcriptions"
    );
    assert_eq!(plan["client_api_format"], "openai:transcription");
    assert_eq!(plan["provider_api_format"], "openai:transcription");
    assert_eq!(plan["stream"], true);
    assert_eq!(
        plan["headers"]["authorization"],
        "Bearer sk-upstream-transcription-stream"
    );
    assert_eq!(plan["headers"]["accept"], "text/event-stream");
    assert!(plan["headers"]["content-type"]
        .as_str()
        .is_some_and(|value| value.contains(&format!("boundary={BOUNDARY}"))));
    assert!(plan["body"]["json_body"].is_null());
    let rewritten = base64::engine::general_purpose::STANDARD
        .decode(
            plan["body"]["body_bytes_b64"]
                .as_str()
                .expect("raw body should be base64 encoded"),
        )
        .expect("raw body should decode");
    let metadata = crate::ai_serving::parse_openai_transcription_request(
        plan["content_type"].as_str(),
        &rewritten,
    )
    .expect("rewritten multipart should parse");
    assert_eq!(metadata.requested_model, PROVIDER_MODEL);
    assert!(metadata.stream);
    assert!(rewritten
        .windows(AUDIO_BYTES.len())
        .any(|window| window == AUDIO_BYTES));

    gateway_handle.abort();
    execution_runtime_handle.abort();
}
