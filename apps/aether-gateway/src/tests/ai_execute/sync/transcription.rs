use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, json, start_server,
    to_bytes, Arc, Body, Json, Mutex, Request, Router, StatusCode,
    EXECUTION_PATH_EXECUTION_RUNTIME_SYNC, EXECUTION_PATH_HEADER, TRACE_ID_HEADER,
};
use aether_crypto::{encrypt_python_fernet_plaintext, DEVELOPMENT_ENCRYPTION_KEY};
use aether_data::repository::auth::{
    InMemoryAuthApiKeySnapshotRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data_contracts::repository::candidate_selection::StoredMinimalCandidateSelectionRow;
use aether_data_contracts::repository::candidates::{
    RequestCandidateReadRepository, RequestCandidateStatus,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use base64::Engine as _;
use sha2::{Digest, Sha256};

const TRANSCRIPTION_SYNC_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const CLIENT_MODEL: &str = "client-transcribe";
const PROVIDER_MODEL: &str = "gpt-4o-transcribe";
const CLIENT_API_KEY: &str = "sk-client-transcription";
const BOUNDARY: &str = "aether-transcription-sync-boundary";
const AUDIO_BYTES: &[u8] = b"\0\xffRIFF\r\n--not-a-real-boundary--\x80audio";

fn run_transcription_sync_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(TRANSCRIPTION_SYNC_TEST_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build")
                .block_on(make_future());
        })
        .expect("transcription sync test thread should spawn");

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
        "user-transcription-1".to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        None,
        Some(json!(["openai:transcription"])),
        None,
        "api-key-transcription-1".to_string(),
        Some("transcription-client".to_string()),
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

fn candidate_row(index: usize) -> StoredMinimalCandidateSelectionRow {
    StoredMinimalCandidateSelectionRow {
        provider_id: format!("provider-transcription-{index}"),
        provider_name: format!("transcription-provider-{index}"),
        provider_type: "custom".to_string(),
        provider_priority: (index as i32) * 10,
        provider_is_active: true,
        endpoint_id: format!("endpoint-transcription-{index}"),
        endpoint_api_format: "openai:transcription".to_string(),
        endpoint_api_family: Some("openai".to_string()),
        endpoint_kind: Some("transcription".to_string()),
        endpoint_is_active: true,
        key_id: format!("key-transcription-{index}"),
        key_name: format!("key-{index}"),
        key_auth_type: "api_key".to_string(),
        key_is_active: true,
        key_api_formats: Some(vec!["openai:transcription".to_string()]),
        key_allowed_models: None,
        key_capabilities: None,
        key_internal_priority: index as i32,
        key_global_priority_by_format: Some(json!({"openai:transcription": index})),
        model_id: format!("model-transcription-{index}"),
        global_model_id: "global-model-transcription-1".to_string(),
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

fn provider(index: usize) -> StoredProviderCatalogProvider {
    StoredProviderCatalogProvider::new(
        format!("provider-transcription-{index}"),
        format!("transcription-provider-{index}"),
        Some(format!("https://provider-{index}.example")),
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

fn endpoint(index: usize) -> StoredProviderCatalogEndpoint {
    StoredProviderCatalogEndpoint::new(
        format!("endpoint-transcription-{index}"),
        format!("provider-transcription-{index}"),
        "openai:transcription".to_string(),
        Some("openai".to_string()),
        Some("transcription".to_string()),
        true,
    )
    .expect("endpoint should build")
    .with_transport_fields(
        format!("https://provider-{index}.example/v1"),
        Some(json!([{
            "action": "set",
            "key": "x-provider-tag",
            "value": format!("transcription-{index}")
        }])),
        None,
        Some(1),
        Some("/speech/transcribe".to_string()),
        None,
        None,
        None,
    )
    .expect("endpoint transport should build")
}

fn key(index: usize) -> StoredProviderCatalogKey {
    StoredProviderCatalogKey::new(
        format!("key-transcription-{index}"),
        format!("provider-transcription-{index}"),
        format!("key-{index}"),
        "api_key".to_string(),
        None,
        true,
    )
    .expect("key should build")
    .with_transport_fields(
        Some(json!(["openai:transcription"])),
        encrypt_python_fernet_plaintext(
            DEVELOPMENT_ENCRYPTION_KEY,
            &format!("sk-upstream-transcription-{index}"),
        )
        .expect("upstream key should encrypt"),
        None,
        None,
        Some(json!({"openai:transcription": index})),
        None,
        None,
        None,
        None,
    )
    .expect("key transport should build")
}

fn multipart_body(stream: Option<bool>, response_format: &str) -> Vec<u8> {
    let mut body = Vec::new();
    let mut push_text = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    };
    push_text("model", CLIENT_MODEL);
    push_text("response_format", response_format);
    push_text("language", "en");
    push_text("future_field", "preserve-me");
    if let Some(stream) = stream {
        push_text("stream", if stream { "true" } else { "false" });
    }
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
    );
    body.extend_from_slice(AUDIO_BYTES);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

#[test]
fn gateway_preserves_transcription_sync_binary_body_response_formats_and_failover() {
    run_transcription_sync_test(
        "gateway_preserves_transcription_sync_binary_body_response_formats_and_failover",
        gateway_preserves_transcription_sync_binary_body_response_formats_and_failover_impl,
    );
}

async fn gateway_preserves_transcription_sync_binary_body_response_formats_and_failover_impl() {
    let seen_plans = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let seen_plans_clone = Arc::clone(&seen_plans);
    let execution_runtime = Router::new().route(
        "/v1/execute/sync",
        any(move |request: Request| {
            let seen_plans = Arc::clone(&seen_plans_clone);
            async move {
                let (_, body) = request.into_parts();
                let bytes = to_bytes(body, usize::MAX).await.expect("body should read");
                let payload: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("execution payload should parse");
                let request_id = payload["request_id"].as_str().unwrap_or_default().to_string();
                let provider_id = payload["provider_id"].as_str().unwrap_or_default().to_string();
                seen_plans.lock().expect("mutex should lock").push(payload);

                let (status_code, content_type, response_bytes) =
                    if request_id == "trace-transcription-failover-1"
                        && provider_id == "provider-transcription-1"
                    {
                        (
                            500,
                            "application/json",
                            br#"{"error":{"message":"primary unavailable"}}"#.to_vec(),
                        )
                    } else if request_id == "trace-transcription-srt-1"
                        || request_id == "trace-transcription-failover-1"
                    {
                        (
                            200,
                            "application/x-subrip",
                            b"1\n00:00:00,000 --> 00:00:01,000\nhello\n".to_vec(),
                        )
                    } else {
                        (
                            200,
                            "application/json",
                            br#"{"text":"hello","usage":{"type":"tokens","input_tokens":12,"output_tokens":5,"total_tokens":17}}"#.to_vec(),
                        )
                    };
                Json(json!({
                    "request_id": request_id,
                    "status_code": status_code,
                    "headers": {
                        "content-type": content_type,
                        "x-transcription-upstream": provider_id
                    },
                    "body": {
                        "body_bytes_b64": base64::engine::general_purpose::STANDARD.encode(response_bytes)
                    },
                    "telemetry": {"elapsed_ms": 7}
                }))
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key(CLIENT_API_KEY)),
        auth_snapshot(),
    )]));
    let candidate_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            candidate_row(1),
            candidate_row(2),
        ]));
    let catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![provider(1), provider(2)],
        vec![endpoint(1), endpoint(2)],
        vec![key(1), key(2)],
    ));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::default());
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let data_state = crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
        auth_repository,
        candidate_repository,
        catalog_repository,
        Arc::clone(&request_candidates),
        DEVELOPMENT_ENCRYPTION_KEY,
    );
    let state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(data_state);
    let gateway = build_router_with_state(state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let json_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/audio/transcriptions"))
        .header(
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-transcription-json-1")
        .body(multipart_body(Some(false), "json"))
        .send()
        .await
        .expect("JSON transcription request should complete");
    assert_eq!(json_response.status(), StatusCode::OK);
    assert_eq!(
        json_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some(EXECUTION_PATH_EXECUTION_RUNTIME_SYNC)
    );
    assert_eq!(
        json_response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(
        json_response.bytes().await.expect("JSON body should read").as_ref(),
        br#"{"text":"hello","usage":{"type":"tokens","input_tokens":12,"output_tokens":5,"total_tokens":17}}"#
    );

    let srt_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/audio/transcriptions"))
        .header(
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-transcription-srt-1")
        .body(multipart_body(None, "srt"))
        .send()
        .await
        .expect("SRT transcription request should complete");
    assert_eq!(srt_response.status(), StatusCode::OK);
    assert_eq!(
        srt_response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/x-subrip")
    );
    assert_eq!(
        srt_response
            .bytes()
            .await
            .expect("SRT body should read")
            .as_ref(),
        b"1\n00:00:00,000 --> 00:00:01,000\nhello\n"
    );

    let failover_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/audio/transcriptions"))
        .header(
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-transcription-failover-1")
        .body(multipart_body(Some(false), "srt"))
        .send()
        .await
        .expect("failover transcription request should complete");
    assert_eq!(failover_response.status(), StatusCode::OK);
    assert_eq!(
        failover_response
            .headers()
            .get("x-transcription-upstream")
            .and_then(|v| v.to_str().ok()),
        Some("provider-transcription-2")
    );

    let plans = seen_plans.lock().expect("mutex should lock").clone();
    let first_plan = plans
        .iter()
        .find(|plan| plan["request_id"] == "trace-transcription-json-1")
        .expect("JSON plan should be captured");
    assert_eq!(
        first_plan["url"],
        "https://provider-1.example/v1/speech/transcribe"
    );
    assert_eq!(first_plan["client_api_format"], "openai:transcription");
    assert_eq!(first_plan["provider_api_format"], "openai:transcription");
    assert_eq!(first_plan["stream"], false);
    assert_eq!(first_plan["headers"]["x-provider-tag"], "transcription-1");
    assert_eq!(
        first_plan["headers"]["authorization"],
        "Bearer sk-upstream-transcription-1"
    );
    assert!(first_plan["headers"]["content-type"]
        .as_str()
        .is_some_and(|value| value.contains(&format!("boundary={BOUNDARY}"))));
    assert!(first_plan["headers"]["content-length"].is_null());
    assert!(first_plan["body"]["json_body"].is_null());

    let rewritten = base64::engine::general_purpose::STANDARD
        .decode(
            first_plan["body"]["body_bytes_b64"]
                .as_str()
                .expect("raw body should be base64 encoded"),
        )
        .expect("raw body base64 should decode");
    let metadata = crate::ai_serving::parse_openai_transcription_request(
        first_plan["content_type"].as_str(),
        &rewritten,
    )
    .expect("rewritten multipart should parse");
    assert_eq!(metadata.requested_model, PROVIDER_MODEL);
    assert!(!metadata.stream);
    assert!(rewritten
        .windows(AUDIO_BYTES.len())
        .any(|window| window == AUDIO_BYTES));
    assert!(rewritten
        .windows(b"preserve-me".len())
        .any(|window| window == b"preserve-me"));

    let failover_provider_ids = plans
        .iter()
        .filter(|plan| plan["request_id"] == "trace-transcription-failover-1")
        .map(|plan| plan["provider_id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        failover_provider_ids,
        vec![
            json!("provider-transcription-1"),
            json!("provider-transcription-2")
        ]
    );
    let candidates = request_candidates
        .list_by_request_id("trace-transcription-failover-1")
        .await
        .expect("failover candidates should read");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].status, RequestCandidateStatus::Failed);
    assert_eq!(candidates[1].status, RequestCandidateStatus::Success);

    gateway_handle.abort();
    execution_runtime_handle.abort();
}
