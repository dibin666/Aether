use std::sync::Arc;

use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::quota::InMemoryProviderQuotaRepository;
use aether_data_contracts::repository::candidate_selection::StoredProviderModelMapping;
use aether_data_contracts::repository::candidates::{
    RequestCandidateStatus, StoredRequestCandidate,
};
use aether_data_contracts::repository::quota::StoredProviderQuotaSnapshot;

use crate::data::GatewayDataState;
use crate::AppState;

use super::super::runtime::should_skip_provider_quota;
use super::super::selection::select_minimal_candidate as select_candidate;
use super::support::{sample_auth_snapshot, sample_key, sample_provider, sample_row};

#[test]
fn skips_inactive_or_exhausted_monthly_quota_provider() {
    let inactive = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "monthly_quota".to_string(),
        Some(10.0),
        1.0,
        Some(30),
        Some(1_000),
        None,
        false,
    )
    .expect("quota should build");
    assert!(should_skip_provider_quota(&inactive, 2_000));

    let exhausted = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "monthly_quota".to_string(),
        Some(10.0),
        10.0,
        Some(30),
        Some(1_000),
        None,
        true,
    )
    .expect("quota should build");
    assert!(should_skip_provider_quota(&exhausted, 2_000));

    let payg = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "pay_as_you_go".to_string(),
        None,
        10.0,
        None,
        None,
        None,
        true,
    )
    .expect("quota should build");
    assert!(!should_skip_provider_quota(&payg, 2_000));
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_quota_is_exhausted() {
    let mut first = sample_row();
    first.provider_id = "provider-1".to_string();
    first.provider_name = "openai-primary".to_string();
    first.endpoint_id = "endpoint-1".to_string();
    first.key_id = "key-1".to_string();
    first.key_name = "primary".to_string();
    first.model_provider_model_name = "gpt-4.1-primary".to_string();
    first.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-primary".to_string(),
        priority: 1,
        api_formats: Some(vec!["openai:chat".to_string()]),
    }]);
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-2".to_string();
    second.provider_name = "openai-secondary".to_string();
    second.endpoint_id = "endpoint-2".to_string();
    second.key_id = "key-2".to_string();
    second.key_name = "secondary".to_string();
    second.model_provider_model_name = "gpt-4.1-secondary".to_string();
    second.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-secondary".to_string(),
        priority: 1,
        api_formats: Some(vec!["openai:chat".to_string()]),
    }]);
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 2}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![
        StoredProviderQuotaSnapshot::new(
            "provider-1".to_string(),
            "monthly_quota".to_string(),
            Some(10.0),
            10.0,
            Some(30),
            Some(1_000),
            None,
            true,
        )
        .expect("quota should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        2_000,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-2");
    assert_eq!(selected.selected_provider_model_name, "gpt-4.1-secondary");
}

#[tokio::test]
async fn cooled_down_when_recent_failures_are_recorded_for_same_key() {
    let mut first = sample_row();
    first.provider_id = "provider-1".to_string();
    first.provider_name = "openai-primary".to_string();
    first.endpoint_id = "endpoint-1".to_string();
    first.key_id = "key-1".to_string();
    first.key_name = "primary".to_string();
    first.model_provider_model_name = "gpt-4.1-primary".to_string();
    first.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-primary".to_string(),
        priority: 1,
        api_formats: Some(vec!["openai:chat".to_string()]),
    }]);
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-2".to_string();
    second.provider_name = "openai-secondary".to_string();
    second.endpoint_id = "endpoint-2".to_string();
    second.key_id = "key-2".to_string();
    second.key_name = "secondary".to_string();
    second.model_provider_model_name = "gpt-4.1-secondary".to_string();
    second.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-secondary".to_string(),
        priority: 1,
        api_formats: Some(vec!["openai:chat".to_string()]),
    }]);
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 2}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Failed,
            None,
            false,
            Some(502),
            None,
            Some("upstream".to_string()),
            Some(100),
            None,
            None,
            None,
            95,
            Some(95),
            Some(95),
        )
        .expect("candidate should build"),
        StoredRequestCandidate::new(
            "cand-2".to_string(),
            "req-2".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Cancelled,
            None,
            false,
            Some(499),
            None,
            Some("cancelled".to_string()),
            Some(80),
            None,
            None,
            None,
            98,
            Some(98),
            Some(98),
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_quota_and_request_candidates_for_tests(
                candidates,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-2");
    assert_eq!(selected.selected_provider_model_name, "gpt-4.1-secondary");
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_concurrent_limit_is_reached() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 2}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", Some(1)),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        Vec::new(),
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            RequestCandidateStatus::Streaming,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            95,
            Some(95),
            None,
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn returns_none_when_auth_api_key_concurrent_limit_is_reached() {
    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        sample_row(),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-1", None)],
        Vec::new(),
        Vec::new(),
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Pending,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            95,
            Some(95),
            None,
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let mut auth_snapshot = sample_auth_snapshot("api-key-1");
    auth_snapshot.api_key_concurrent_limit = Some(1);

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed");

    assert!(selected.is_none());
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_key_rpm_slots_are_reserved_for_new_user() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 2}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)),
            sample_key("key-b", "provider-b", Some(10)),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            Some("api-key-new-user".to_string()),
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            RequestCandidateStatus::Success,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            Some(9),
            None,
            None,
            95,
            Some(95),
            Some(96),
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let auth_snapshot = sample_auth_snapshot("api-key-new-user");

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_key_circuit_is_open() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 2}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.2}})),
                Some(serde_json::json!({"openai:chat": {"open": true}})),
            ),
            sample_key("key-b", "provider-b", Some(10)),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn same_priority_candidates_prefer_healthier_provider_key_before_id_order() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.30}})),
                None,
            ),
            sample_key("key-b", "provider-b", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.95}})),
                None,
            ),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn same_priority_candidates_use_aggregate_health_score_when_api_format_specific_health_is_missing(
) {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();
    first.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();
    second.key_global_priority_by_format = Some(serde_json::json!({"openai:chat": 1}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({
                    "openai:responses": {"health_score": 0.40},
                    "claude:chat": {"health_score": 0.55}
                })),
                None,
            ),
            sample_key("key-b", "provider-b", Some(10)).with_health_fields(
                Some(serde_json::json!({
                    "openai:responses": {"health_score": 0.90},
                    "claude:chat": {"health_score": 0.92}
                })),
                None,
            ),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}
