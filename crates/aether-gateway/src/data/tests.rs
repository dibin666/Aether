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
    InMemoryShadowResultRepository, RecordShadowResultSample, ShadowResultLookupKey,
    ShadowResultMatchStatus, ShadowResultReadRepository, ShadowResultSampleOrigin,
    UpsertShadowResult,
};
use aether_data::repository::usage::{InMemoryUsageReadRepository, StoredRequestUsageAudit};
use aether_data::repository::video_tasks::{
    InMemoryVideoTaskRepository, UpsertVideoTask, VideoTaskLookupKey, VideoTaskStatus,
    VideoTaskWriteRepository,
};

use super::{GatewayDataConfig, GatewayDataState};
use crate::gateway::AppState;

#[test]
fn disabled_gateway_data_state_has_no_backends() {
    let state = GatewayDataState::from_config(GatewayDataConfig::disabled())
        .expect("disabled config should build");

    assert!(!state.has_backends());
    assert!(!state.has_auth_api_key_reader());
    assert!(!state.has_request_candidate_reader());
    assert!(!state.has_provider_catalog_reader());
    assert!(!state.has_usage_reader());
    assert!(!state.has_video_task_reader());
    assert!(!state.has_shadow_result_reader());
    assert!(!state.has_shadow_result_writer());
}

#[tokio::test]
async fn postgres_gateway_data_state_builds_video_task_reader() {
    let state = GatewayDataState::from_config(GatewayDataConfig::from_postgres_url(
        "postgres://localhost/aether",
        false,
    ))
    .expect("postgres-backed state should build");

    assert!(state.has_backends());
    assert!(state.has_auth_api_key_reader());
    assert!(state.has_request_candidate_reader());
    assert!(state.has_provider_catalog_reader());
    assert!(state.has_usage_reader());
    assert!(state.has_video_task_reader());
    assert!(state.has_shadow_result_reader());
    assert!(state.has_shadow_result_writer());
}

#[tokio::test]
async fn data_state_find_uses_configured_read_repository() {
    let repository = Arc::new(InMemoryVideoTaskRepository::default());
    repository
        .upsert(UpsertVideoTask {
            id: "task-1".to_string(),
            short_id: Some("short-task-1".to_string()),
            user_id: Some("user-1".to_string()),
            external_task_id: Some("ext-task-1".to_string()),
            provider_api_format: Some("openai:video".to_string()),
            model: Some("sora-2".to_string()),
            prompt: Some("hello".to_string()),
            size: Some("1280x720".to_string()),
            status: VideoTaskStatus::Queued,
            progress_percent: 0,
            created_at_unix_secs: 100,
            updated_at_unix_secs: 100,
            error_code: None,
            error_message: None,
            video_url: None,
        })
        .await
        .expect("upsert should succeed");

    let state = GatewayDataState::with_video_task_reader_for_tests(repository);

    let task = state
        .find_video_task(VideoTaskLookupKey::Id("task-1"))
        .await
        .expect("find should succeed");

    assert_eq!(task.expect("task should exist").id, "task-1");
}

#[tokio::test]
async fn app_state_wires_gateway_data_state_from_config() {
    let state = AppState::new_with_executor(
        "http://127.0.0.1:18084",
        Some("http://127.0.0.1:18085".to_string()),
        Some("http://127.0.0.1:18086".to_string()),
    )
    .expect("app state should build")
    .with_data_config(GatewayDataConfig::from_postgres_url(
        "postgres://localhost/aether",
        false,
    ))
    .expect("data config should wire");

    assert!(state.data.has_backends());
    assert!(state.data.has_auth_api_key_reader());
    assert!(state.data.has_request_candidate_reader());
    assert!(state.data.has_provider_catalog_reader());
    assert!(state.data.has_usage_reader());
    assert!(state.data.has_video_task_reader());
    assert!(state.data.has_shadow_result_reader());
    assert!(state.data.has_shadow_result_writer());
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
        Some(200),
        Some(serde_json::json!(["openai"])),
        Some(serde_json::json!(["openai:chat"])),
        Some(serde_json::json!(["gpt-4.1"])),
    )
    .expect("auth snapshot should build")
}

#[tokio::test]
async fn data_state_reads_auth_api_key_snapshot_from_reader() {
    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_auth_snapshot("key-1", "user-1"),
    )]));
    let state = GatewayDataState::with_auth_api_key_reader_for_tests(repository);

    let snapshot = state
        .read_auth_api_key_snapshot("user-1", "key-1", 150)
        .await
        .expect("read should succeed")
        .expect("snapshot should exist");

    assert_eq!(snapshot.user_id, "user-1");
    assert_eq!(snapshot.api_key_id, "key-1");
    assert_eq!(snapshot.username, "alice");
    assert_eq!(
        snapshot.api_key_allowed_models,
        Some(vec!["gpt-4.1".to_string()])
    );
    assert!(snapshot.currently_usable);
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
async fn data_state_reads_decision_trace_with_provider_catalog_metadata() {
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            Some("alice".to_string()),
            Some("default".to_string()),
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("provider-key-1".to_string()),
            RequestCandidateStatus::Failed,
            None,
            false,
            Some(502),
            None,
            None,
            Some(37),
            Some(1),
            None,
            Some(serde_json::json!({"cache_1h": true})),
            100,
            Some(101),
            Some(102),
        )
        .expect("candidate should build"),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));
    let state = GatewayDataState::with_decision_trace_readers_for_tests(
        request_candidates,
        provider_catalog,
    );

    let trace = state
        .read_decision_trace("req-1", true)
        .await
        .expect("trace should read")
        .expect("trace should exist");

    assert_eq!(trace.request_id, "req-1");
    assert_eq!(trace.total_candidates, 1);
    assert_eq!(trace.candidates[0].provider_name.as_deref(), Some("OpenAI"));
    assert_eq!(
        trace.candidates[0].endpoint_api_format.as_deref(),
        Some("openai:chat")
    );
    assert_eq!(
        trace.candidates[0].provider_key_auth_type.as_deref(),
        Some("api_key")
    );
    assert_eq!(
        trace.candidates[0].provider_key_capabilities,
        Some(serde_json::json!({"cache_1h": true}))
    );
}

#[tokio::test]
async fn data_state_reads_request_usage_audit_from_reader() {
    let repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_request_usage("req-usage-1"),
    ]));
    let state = GatewayDataState::with_usage_reader_for_tests(repository);

    let usage = state
        .read_request_usage_audit("req-usage-1")
        .await
        .expect("read should succeed")
        .expect("usage should exist");

    assert_eq!(usage.usage.request_id, "req-usage-1");
    assert_eq!(usage.usage.provider_name, "OpenAI");
    assert_eq!(usage.usage.total_tokens, 160);
    assert_eq!(usage.usage.total_cost_usd, 0.24);
    assert!(usage.usage.has_format_conversion);
}

#[tokio::test]
async fn data_state_reads_request_audit_bundle_from_multiple_readers() {
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some("hash-1".to_string()),
        sample_auth_snapshot("api-key-1", "user-1"),
    )]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-usage-1".to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            Some("alice".to_string()),
            Some("default".to_string()),
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("provider-key-1".to_string()),
            RequestCandidateStatus::Success,
            None,
            false,
            Some(200),
            None,
            None,
            Some(37),
            Some(1),
            None,
            Some(serde_json::json!({"cache_1h": true})),
            100,
            Some(101),
            Some(102),
        )
        .expect("candidate should build"),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider_catalog_provider()],
        vec![sample_provider_catalog_endpoint()],
        vec![sample_provider_catalog_key()],
    ));
    let usage_repository = Arc::new(InMemoryUsageReadRepository::seed(vec![
        sample_request_usage("req-usage-1"),
    ]));
    let state = GatewayDataState::with_request_audit_readers_for_tests(
        auth_repository,
        request_candidates,
        provider_catalog,
        usage_repository,
    );

    let bundle = state
        .read_request_audit_bundle("req-usage-1", true, 150)
        .await
        .expect("bundle should read")
        .expect("bundle should exist");

    assert_eq!(bundle.request_id, "req-usage-1");
    assert_eq!(
        bundle
            .usage
            .as_ref()
            .and_then(|usage| usage.usage.target_model.as_deref()),
        Some("gpt-4.1-mini")
    );
    assert_eq!(
        bundle
            .decision_trace
            .as_ref()
            .and_then(|trace| trace.candidates.first())
            .and_then(|candidate| candidate.provider_name.as_deref()),
        Some("OpenAI")
    );
    assert_eq!(
        bundle
            .auth_snapshot
            .as_ref()
            .map(|snapshot| snapshot.currently_usable),
        Some(true)
    );
}

#[tokio::test]
async fn maps_openai_video_task_repository_row_into_read_response() {
    let repository = Arc::new(InMemoryVideoTaskRepository::default());
    repository
        .upsert(UpsertVideoTask {
            id: "task-1".to_string(),
            short_id: Some("short-task-1".to_string()),
            user_id: Some("user-1".to_string()),
            external_task_id: Some("ext-task-1".to_string()),
            provider_api_format: Some("openai:video".to_string()),
            model: Some("sora-2".to_string()),
            prompt: Some("hello".to_string()),
            size: Some("1280x720".to_string()),
            status: VideoTaskStatus::Processing,
            progress_percent: 45,
            created_at_unix_secs: 100,
            updated_at_unix_secs: 120,
            error_code: None,
            error_message: None,
            video_url: None,
        })
        .await
        .expect("upsert should succeed");

    let state = GatewayDataState::with_video_task_reader_for_tests(repository);
    let response = state
        .read_video_task_response(Some("openai"), "/v1/videos/task-1")
        .await
        .expect("read should succeed")
        .expect("read response should exist");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body_json["id"], "task-1");
    assert_eq!(response.body_json["status"], "processing");
    assert_eq!(response.body_json["created_at"], 100);
}

#[tokio::test]
async fn maps_gemini_video_task_repository_row_into_read_response() {
    let repository = Arc::new(InMemoryVideoTaskRepository::default());
    repository
        .upsert(UpsertVideoTask {
            id: "task-1".to_string(),
            short_id: Some("localshort123".to_string()),
            user_id: Some("user-1".to_string()),
            external_task_id: Some("operations/ext-task-1".to_string()),
            provider_api_format: Some("gemini:video".to_string()),
            model: Some("veo-3".to_string()),
            prompt: Some("hello".to_string()),
            size: Some("720p".to_string()),
            status: VideoTaskStatus::Completed,
            progress_percent: 100,
            created_at_unix_secs: 100,
            updated_at_unix_secs: 120,
            error_code: None,
            error_message: None,
            video_url: None,
        })
        .await
        .expect("upsert should succeed");

    let state = GatewayDataState::with_video_task_reader_for_tests(repository);
    let response = state
        .read_video_task_response(
            Some("gemini"),
            "/v1beta/models/veo-3/operations/localshort123",
        )
        .await
        .expect("read should succeed")
        .expect("read response should exist");

    assert_eq!(response.status_code, 200);
    assert_eq!(
        response.body_json["name"],
        "models/veo-3/operations/localshort123"
    );
    assert_eq!(response.body_json["done"], true);
}

#[tokio::test]
async fn data_state_write_uses_configured_shadow_result_writer() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());
    let state = GatewayDataState::with_shadow_result_writer_for_tests(repository.clone());

    let written = state
        .write_shadow_result(UpsertShadowResult {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            rust_result_digest: Some("rust-digest".to_string()),
            python_result_digest: None,
            match_status: ShadowResultMatchStatus::Pending,
            status_code: Some(200),
            error_message: None,
            created_at_unix_secs: 100,
            updated_at_unix_secs: 100,
        })
        .await
        .expect("write should succeed");

    assert!(written.is_some());
    let stored = repository
        .find(ShadowResultLookupKey::TraceFingerprint {
            trace_id: "trace-1",
            request_fingerprint: "fp-1",
        })
        .await
        .expect("find should succeed");
    assert_eq!(
        stored.expect("stored result should exist").match_status,
        ShadowResultMatchStatus::Pending
    );
}

#[tokio::test]
async fn data_state_records_shadow_result_samples_and_merges_match_status() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());
    let state = GatewayDataState::with_shadow_result_repository_for_tests(repository);

    let first = state
        .record_shadow_result_sample(RecordShadowResultSample {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            origin: ShadowResultSampleOrigin::Rust,
            result_digest: "digest-1".to_string(),
            status_code: Some(200),
            error_message: None,
            recorded_at_unix_secs: 100,
        })
        .await
        .expect("first record should succeed")
        .expect("first stored result should exist");
    assert_eq!(first.match_status, ShadowResultMatchStatus::Pending);

    let second = state
        .record_shadow_result_sample(RecordShadowResultSample {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            origin: ShadowResultSampleOrigin::Python,
            result_digest: "digest-1".to_string(),
            status_code: Some(200),
            error_message: None,
            recorded_at_unix_secs: 200,
        })
        .await
        .expect("second record should succeed")
        .expect("second stored result should exist");

    assert_eq!(second.match_status, ShadowResultMatchStatus::Match);
    assert_eq!(second.created_at_unix_secs, 100);
    assert_eq!(second.updated_at_unix_secs, 200);
    assert_eq!(second.request_id.as_deref(), Some("req-1"));
}

#[tokio::test]
async fn data_state_lists_recent_shadow_results_from_reader() {
    let repository = Arc::new(InMemoryShadowResultRepository::default());
    let state = GatewayDataState::with_shadow_result_repository_for_tests(repository.clone());

    state
        .record_shadow_result_sample(RecordShadowResultSample {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-shadow-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            origin: ShadowResultSampleOrigin::Rust,
            result_digest: "digest-1".to_string(),
            status_code: Some(200),
            error_message: None,
            recorded_at_unix_secs: 100,
        })
        .await
        .expect("record should succeed");

    let recent = state
        .list_recent_shadow_results(5)
        .await
        .expect("list recent should succeed");

    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].trace_id, "trace-1");
    assert_eq!(recent[0].request_id.as_deref(), Some("req-shadow-1"));
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

#[tokio::test]
async fn data_state_reads_request_candidate_trace_from_reader() {
    let repository = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        sample_request_candidate(
            "cand-1",
            "req-1",
            0,
            RequestCandidateStatus::Pending,
            None,
            None,
            None,
        ),
        sample_request_candidate(
            "cand-2",
            "req-1",
            1,
            RequestCandidateStatus::Success,
            Some(101),
            Some(42),
            Some(200),
        ),
    ]));
    let state = GatewayDataState::with_request_candidate_reader_for_tests(repository);

    let trace = state
        .read_request_candidate_trace("req-1", true)
        .await
        .expect("trace should succeed")
        .expect("trace should exist");

    assert_eq!(trace.request_id, "req-1");
    assert_eq!(trace.total_candidates, 1);
    assert_eq!(
        trace.final_status,
        super::candidates::RequestCandidateFinalStatus::Success
    );
    assert_eq!(trace.total_latency_ms, 42);
    assert_eq!(trace.candidates[0].id, "cand-2");
}
