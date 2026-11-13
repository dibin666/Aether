//! End-to-end coverage for every Decision API dialect the gateway exposes.
//!
//! Contracts follow the official documentation exactly:
//! - Gemini Decisions API (`POST /v1beta/decisions`, `input` + `questions` map,
//!   `noul`/`choice`/`score`): <https://aistudio.google.com/docs/decisions-api>
//! - OpenAI Decisions API (`POST /v1/decisions`, `input` + `questions` array,
//!   `predicate`/`choice`/`score`): <https://platform.openai.com/docs/guides/decisions>
//! - System One (`POST /v1/systemone`, `state` + `questions` map,
//!   `noul`/`choice`/`score`): <https://openrouter.ai/docs/api/reference/decisions>

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
use sha2::{Digest, Sha256};

const DECISIONS_SYNC_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const CLIENT_MODEL: &str = "decision-client-model";
const CLIENT_API_KEY: &str = "sk-client-decisions";

#[test]
fn gateway_converts_decisions_pairs_and_enforces_conversion_gate() {
    run_decisions_sync_test(
        "gateway_converts_decisions_pairs_and_enforces_conversion_gate",
        || async {
            let formats = [
                "gemini:decisions",
                "openai:decisions",
                "systemone:decisions",
            ];
            for client in formats {
                for upstream in formats {
                    if client == upstream {
                        continue;
                    }
                    assert_cross_format_route(client, upstream, true, false).await;
                }
            }
            assert_cross_format_route("openai:decisions", "gemini:decisions", false, false).await;
            assert_cross_format_route("openai:decisions", "gemini:decisions", false, true).await;
        },
    );
}

async fn assert_cross_format_route(
    client: &str,
    upstream: &str,
    enabled: bool,
    endpoint_enabled: bool,
) {
    let seen = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let capture = Arc::clone(&seen);
    let runtime=Router::new().route("/v1/execute/sync",any(move |request:Request| {
        let capture=Arc::clone(&capture);
        async move {
            let bytes=to_bytes(request.into_body(),usize::MAX).await.unwrap();
            let plan:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
            let response=if plan["provider_api_format"]=="openai:decisions" {
                json!({"model":"gemini-decision-1","answers":[{"name":"team","type":"choice","choice":"billing","confidence":0.9,"probabilities":[{"value":"billing","probability":0.9},{"value":"shipping","probability":0.1}]}],"usage":{"input_tokens":23,"output_tokens":0,"total_tokens":23}})
            } else {
                json!({"model":"gemini-decision-1","answers":{"team":{"type":"choice","choice":"billing","confidence":0.9,"probabilities":{"billing":0.9,"shipping":0.1}}},"usage":if plan["provider_api_format"]=="gemini:decisions" {json!({"total_input_tokens":23,"total_output_tokens":0,"total_tokens":23})} else {json!({"input_tokens":23,"cached_input_tokens":0,"output_tokens":0})}})
            };
            let request_id=plan["request_id"].clone();capture.lock().unwrap().push(plan);
            Json(json!({"request_id":request_id,"status_code":200,"headers":{"content-type":"application/json"},"body":{"json_body":response},"telemetry":{"elapsed_ms":5}}))
        }
    }));
    let (runtime_url, runtime_handle) = start_server(runtime).await;
    let mut upstream_provider = provider(1);
    upstream_provider.enable_format_conversion = enabled;
    let mut upstream_endpoint = endpoint(1, upstream);
    if endpoint_enabled {
        upstream_endpoint.format_acceptance_config =
            Some(json!({"enabled":true,"accept_formats":[client]}));
    }
    let data_state=crate::data::GatewayDataState::with_auth_candidate_selection_provider_catalog_and_request_candidate_repository_for_tests(
        Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(Some(hash_api_key(CLIENT_API_KEY)),auth_snapshot())])),
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![candidate_row(1,upstream)])),
        Arc::new(InMemoryProviderCatalogReadRepository::seed(vec![upstream_provider],vec![upstream_endpoint],vec![key(1,upstream)])),
        Arc::new(InMemoryRequestCandidateRepository::default()),DEVELOPMENT_ENCRYPTION_KEY,
    );
    let state = build_state_with_execution_runtime_override(runtime_url)
        .with_data_state_for_tests(data_state);
    let (url, handle) = start_server(build_router_with_state(state)).await;
    let body = if client == "openai:decisions" {
        json!({"model":CLIENT_MODEL,"input":"A billing question","questions":[{"name":"team","type":"choice","instructions":"Which team?","choices":[{"value":"billing","description":"Payments"},{"value":"shipping","description":"Delivery"}]}]})
    } else {
        json!({"model":CLIENT_MODEL,if client=="systemone:decisions" {"state"} else {"input"}:"A billing question","questions":{"team":{"type":"choice","instructions":"Which team?","criteria":{"billing":"Payments","shipping":"Delivery"}}}})
    };
    let path = match client {
        "gemini:decisions" => "/v1beta/decisions",
        "openai:decisions" => "/v1/decisions",
        _ => "/v1/systemone",
    };
    let mut request = reqwest::Client::new()
        .post(format!("{url}{path}"))
        .header(TRACE_ID_HEADER, "trace-decisions-cross")
        .json(&body);
    request = if client == "gemini:decisions" {
        request.header("x-goog-api-key", CLIENT_API_KEY)
    } else {
        request.bearer_auth(CLIENT_API_KEY)
    };
    let response = request.send().await.unwrap();
    let status = response.status();
    let payload: serde_json::Value = response.json().await.unwrap();
    if enabled || endpoint_enabled {
        assert_eq!(status, StatusCode::OK, "{client} -> {upstream}: {payload}");
        if client == "openai:decisions" {
            assert_eq!(payload["answers"][0]["name"], "team");
            assert_eq!(payload["answers"][0]["choice"], "billing");
        } else {
            assert_eq!(payload["answers"]["team"]["choice"], "billing");
        }
        let plans = seen.lock().unwrap();
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        assert_eq!(plan["provider_api_format"], upstream);
        assert_eq!(plan["client_api_format"], client);
        assert_eq!(plan["stream"], false);
        assert_eq!(plan["body"]["json_body"]["model"], provider_model(1));
        assert_eq!(
            plan["body"]["json_body"][if upstream == "systemone:decisions" {
                "state"
            } else {
                "input"
            }],
            "A billing question"
        );
        if upstream == "gemini:decisions" {
            assert_eq!(plan["headers"]["x-goog-api-key"], "sk-upstream-decisions-1");
        } else {
            assert_eq!(
                plan["headers"]["authorization"],
                "Bearer sk-upstream-decisions-1"
            );
        }
    } else {
        assert!(
            status.is_client_error() || status.is_server_error(),
            "{status}: {payload}"
        );
        assert!(
            seen.lock().unwrap().is_empty(),
            "disabled conversion must not execute upstream"
        );
    }
    if client == "openai:decisions" && (enabled || endpoint_enabled) {
        let mut invalid = body.clone();
        invalid["questions"]
            .as_array_mut()
            .unwrap()
            .push(body["questions"][0].clone());
        let response = reqwest::Client::new()
            .post(format!("{url}{path}"))
            .bearer_auth(CLIENT_API_KEY)
            .json(&invalid)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let error = response.text().await.unwrap();
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert!(error.contains("duplicate"), "{error}");
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "invalid conversion must not execute upstream"
        );
    }
    handle.abort();
    runtime_handle.abort();
}

fn run_decisions_sync_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(DECISIONS_SYNC_TEST_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build")
                .block_on(make_future());
        })
        .expect("decisions sync test thread should spawn");

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
    let api_formats = json!([
        "gemini:decisions",
        "openai:decisions",
        "systemone:decisions",
    ]);
    StoredAuthApiKeySnapshot::new(
        "user-decisions-1".to_string(),
        "alice".to_string(),
        Some("alice@example.com".to_string()),
        "user".to_string(),
        "local".to_string(),
        true,
        false,
        None,
        Some(api_formats.clone()),
        None,
        "api-key-decisions-1".to_string(),
        Some("decisions-client".to_string()),
        true,
        false,
        false,
        Some(60),
        Some(5),
        Some(4_102_444_800_i64),
        None,
        Some(api_formats),
        None,
    )
    .expect("auth snapshot should build")
}

fn candidate_row(index: usize, api_format: &str) -> StoredMinimalCandidateSelectionRow {
    let kind = api_format
        .split_once(':')
        .expect("api format should carry a kind")
        .1;
    StoredMinimalCandidateSelectionRow {
        provider_id: format!("provider-decisions-{index}"),
        provider_name: format!("decisions-provider-{index}"),
        provider_type: "custom".to_string(),
        provider_priority: (index as i32) * 10,
        provider_is_active: true,
        endpoint_id: format!("endpoint-decisions-{index}"),
        endpoint_api_format: api_format.to_string(),
        endpoint_api_family: Some(api_format.split_once(':').expect("family").0.to_string()),
        endpoint_kind: Some(kind.to_string()),
        endpoint_is_active: true,
        key_id: format!("key-decisions-{index}"),
        key_name: format!("key-{index}"),
        key_auth_type: "api_key".to_string(),
        key_is_active: true,
        key_api_formats: Some(vec![api_format.to_string()]),
        key_allowed_models: None,
        key_capabilities: None,
        key_internal_priority: index as i32,
        key_global_priority_by_format: Some(serde_json::Value::Object(serde_json::Map::from_iter(
            [(api_format.to_string(), json!(index))],
        ))),
        model_id: format!("model-decisions-{index}"),
        global_model_id: "global-model-decisions-1".to_string(),
        global_model_name: CLIENT_MODEL.to_string(),
        global_model_mappings: None,
        global_model_supports_streaming: Some(false),
        model_provider_model_name: provider_model(index).to_string(),
        model_provider_model_mappings: None,
        model_supports_streaming: Some(false),
        model_is_active: true,
        model_is_available: true,
    }
}

fn provider_model(index: usize) -> &'static str {
    match index {
        1 => "gemini-decision-1",
        2 => "gpt-6-luna",
        3 => "typesafe/jev-1.13",
        _ => unreachable!(),
    }
}

fn provider(index: usize) -> StoredProviderCatalogProvider {
    StoredProviderCatalogProvider::new(
        format!("provider-decisions-{index}"),
        format!("decisions-provider-{index}"),
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

fn endpoint(index: usize, api_format: &str) -> StoredProviderCatalogEndpoint {
    StoredProviderCatalogEndpoint::new(
        format!("endpoint-decisions-{index}"),
        format!("provider-decisions-{index}"),
        api_format.to_string(),
        Some(api_format.split_once(':').expect("family").0.to_string()),
        Some(api_format.split_once(':').expect("kind").1.to_string()),
        true,
    )
    .expect("endpoint should build")
    .with_transport_fields(
        format!("https://provider-{index}.example"),
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

fn key(index: usize, api_format: &str) -> StoredProviderCatalogKey {
    StoredProviderCatalogKey::new(
        format!("key-decisions-{index}"),
        format!("provider-decisions-{index}"),
        format!("key-{index}"),
        "api_key".to_string(),
        None,
        true,
    )
    .expect("key should build")
    .with_transport_fields(
        Some(json!([api_format])),
        encrypt_python_fernet_plaintext(
            DEVELOPMENT_ENCRYPTION_KEY,
            &format!("sk-upstream-decisions-{index}"),
        )
        .expect("upstream key should encrypt"),
        None,
        None,
        Some(serde_json::Value::Object(serde_json::Map::from_iter([(
            api_format.to_string(),
            json!(index),
        )]))),
        None,
        Some(4_102_444_800),
        None,
        None,
    )
    .expect("key transport should build")
}

#[test]
fn gateway_executes_all_decisions_dialects_with_documented_contracts() {
    run_decisions_sync_test(
        "gateway_executes_all_decisions_dialects_with_documented_contracts",
        gateway_executes_all_decisions_dialects_with_documented_contracts_impl,
    );
}

async fn gateway_executes_all_decisions_dialects_with_documented_contracts_impl() {
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
                let request_id = payload["request_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                seen_plans.lock().expect("mutex should lock").push(payload);

                // Doc-shaped responses per dialect; the gateway must pass them
                // through byte-for-byte on this same-format sync path.
                let upstream_body = match request_id.as_str() {
                    "trace-decisions-gemini-1" => json!({
                        "answers": {
                            "is_damaged": { "type": "noul", "noul": 0.99, "confidence": 0.99 },
                            "department": {
                                "type": "choice",
                                "choice": "billing",
                                "probabilities": { "billing": 0.99, "shipping": 0.01 },
                                "confidence": 0.99
                            },
                            "severity": {
                                "type": "score",
                                "score": 1.12,
                                "legend": {
                                    "0": "Cosmetic: appearance only.",
                                    "1": "Workaround available.",
                                    "2": "Fully blocked."
                                },
                                "probabilities": { "0": 0.03, "1": 0.83, "2": 0.14 },
                                "confidence": 0.83
                            }
                        },
                        "usage": {
                            "total_input_tokens": 100,
                            "input_tokens_by_modality": [
                                { "modality": "text", "tokens": 100 }
                            ],
                            "total_output_tokens": 0,
                            "total_tokens": 100
                        }
                    }),
                    "trace-decisions-openai-1" => json!({
                        "id": "dec_openai_1",
                        "model": "gpt-6-luna",
                        "answers": [
                            {
                                "type": "predicate",
                                "name": "duplicate_charge",
                                "probability": 0.98
                            },
                            {
                                "type": "choice",
                                "name": "department",
                                "choice": "billing",
                                "probabilities": [
                                    { "value": "billing", "probability": 0.97 },
                                    { "value": "technical", "probability": 0.03 }
                                ],
                                "confidence": 0.97
                            },
                            {
                                "type": "score",
                                "name": "severity",
                                "score": 1.4,
                                "confidence": 0.9
                            }
                        ],
                        "usage": {
                            "input_tokens": 120,
                            "output_tokens": 0,
                            "total_tokens": 120
                        }
                    }),
                    _ => json!({
                        "id": "dec_systemone_1",
                        "model": "typesafe/jev-1.13",
                        "answers": {
                            "is_bug": { "type": "noul", "noul": 0.96 },
                            "team": {
                                "type": "choice",
                                "choice": "payments",
                                "confidence": 0.75,
                                "probabilities": { "frontend": 0.25, "payments": 0.75 }
                            },
                            "urgency": {
                                "type": "score",
                                "score": 1.4,
                                "confidence": 0.9,
                                "legend": {
                                    "0": "Can wait for the next release",
                                    "1": "Should be fixed this week",
                                    "2": "Blocking revenue right now"
                                },
                                "probabilities": { "0": 0.1, "1": 0.5, "2": 0.4 }
                            }
                        },
                        "usage": { "input_tokens": 118, "output_tokens": 24 }
                    }),
                };
                (
                    StatusCode::OK,
                    Json(json!({
                        "request_id": request_id,
                        "status_code": 200,
                        "headers": { "content-type": "application/json" },
                        "body": { "json_body": upstream_body },
                        "telemetry": {"elapsed_ms": 5}
                    })),
                )
            }
        }),
    );

    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key(CLIENT_API_KEY)),
        auth_snapshot(),
    )]));
    let candidate_repository =
        Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            candidate_row(1, "gemini:decisions"),
            candidate_row(2, "openai:decisions"),
            candidate_row(3, "systemone:decisions"),
        ]));
    let catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![provider(1), provider(2), provider(3)],
        vec![
            endpoint(1, "gemini:decisions"),
            endpoint(2, "openai:decisions"),
            endpoint(3, "systemone:decisions"),
        ],
        vec![
            key(1, "gemini:decisions"),
            key(2, "openai:decisions"),
            key(3, "systemone:decisions"),
        ],
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

    // 1) Gemini Decisions API — POST /v1beta/decisions, input + questions map.
    let gemini_body = json!({
        "model": CLIENT_MODEL,
        "input": "The blue mug I ordered on Monday arrived cracked on Wednesday.",
        "questions": {
            "is_damaged": {
                "type": "noul",
                "instructions": "Did the item arrive damaged?"
            },
            "department": {
                "type": "choice",
                "instructions": "Which department should handle this request?",
                "criteria": {
                    "billing": "Payments, invoices, and refunds.",
                    "shipping": "Delivery and tracking."
                }
            },
            "severity": {
                "type": "score",
                "instructions": "Rate the severity.",
                "criteria": [
                    "Cosmetic: appearance only.",
                    "Workaround available.",
                    "Fully blocked."
                ]
            }
        }
    });
    let gemini_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1beta/decisions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header("x-goog-api-key", CLIENT_API_KEY)
        .header(TRACE_ID_HEADER, "trace-decisions-gemini-1")
        .json(&gemini_body)
        .send()
        .await
        .expect("gemini decisions request should complete");
    let gemini_status = gemini_response.status();
    if gemini_status != StatusCode::OK {
        let dependency_reason = gemini_response
            .headers()
            .get("x-aether-dependency-reason")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let miss_reason = gemini_response
            .headers()
            .get("x-aether-local-execution-runtime-miss-reason")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = gemini_response.text().await.unwrap_or_default();
        panic!("gemini decisions expected 200, got {gemini_status}; dependency={dependency_reason:?} miss={miss_reason:?} body={body}");
    }
    assert_eq!(
        gemini_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some(EXECUTION_PATH_EXECUTION_RUNTIME_SYNC)
    );
    let gemini_payload: serde_json::Value = gemini_response
        .json()
        .await
        .expect("gemini decisions body should parse");
    assert_eq!(gemini_payload["answers"]["is_damaged"]["noul"], 0.99);
    assert_eq!(gemini_payload["usage"]["total_output_tokens"], 0);

    // 2) OpenAI Decisions API — POST /v1/decisions, input + questions array
    // with predicate/choice/score shapes from the official reference.
    let openai_body = json!({
        "model": CLIENT_MODEL,
        "input": "I was charged twice for my order.",
        "questions": [
            {
                "type": "predicate",
                "name": "duplicate_charge",
                "instructions": "Does the ticket describe a duplicate charge supported by the order?"
            },
            {
                "type": "choice",
                "name": "department",
                "instructions": "Which department should handle this complaint?",
                "choices": [
                    { "value": "billing", "description": "Payments, invoices, and refunds." },
                    { "value": "technical", "description": "Problems using the product." }
                ]
            },
            {
                "type": "score",
                "name": "severity",
                "instructions": "Rate the severity of this issue for the user.",
                "levels": [
                    { "label": "Cosmetic", "description": "Appearance only." },
                    { "label": "Workaround", "description": "A task fails but another way works." },
                    { "label": "Blocked", "description": "No workaround exists." }
                ]
            }
        ]
    });
    let openai_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/decisions"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-decisions-openai-1")
        .json(&openai_body)
        .send()
        .await
        .expect("openai decisions request should complete");
    assert_eq!(openai_response.status(), StatusCode::OK);
    assert_eq!(
        openai_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some(EXECUTION_PATH_EXECUTION_RUNTIME_SYNC)
    );
    let openai_payload: serde_json::Value = openai_response
        .json()
        .await
        .expect("openai decisions body should parse");
    assert_eq!(openai_payload["answers"][0]["name"], "duplicate_charge");
    assert_eq!(openai_payload["answers"][1]["choice"], "billing");

    // 3) System One — POST /v1/systemone, state + questions map (OpenRouter
    // / TypeSafe Jev contract).
    let systemone_body = json!({
        "model": CLIENT_MODEL,
        "state": {
            "customer_tier": "enterprise",
            "ticket": "My checkout page shows a blank screen after I click Pay."
        },
        "questions": {
            "is_bug": {
                "type": "noul",
                "instructions": "Is the customer reporting a software defect?",
                "criteria": {
                    "true": "The customer describes broken or unexpected product behavior.",
                    "false": "The customer is asking a question or requesting a feature."
                }
            },
            "team": {
                "type": "choice",
                "instructions": "Which team should own this ticket?",
                "criteria": {
                    "frontend": "Rendering, layout, or browser compatibility issues.",
                    "payments": "Checkout, billing, or payment processing issues."
                }
            },
            "urgency": {
                "type": "score",
                "instructions": "How urgent is this ticket?",
                "criteria": [
                    "Can wait for the next release",
                    "Should be fixed this week",
                    "Blocking revenue right now"
                ]
            }
        }
    });
    let systemone_response = reqwest::Client::new()
        .post(format!("{gateway_url}/v1/systemone"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {CLIENT_API_KEY}"),
        )
        .header(TRACE_ID_HEADER, "trace-decisions-systemone-1")
        .json(&systemone_body)
        .send()
        .await
        .expect("systemone decisions request should complete");
    let systemone_status = systemone_response.status();
    if systemone_status != StatusCode::OK {
        let dependency_reason = systemone_response
            .headers()
            .get("x-aether-dependency-reason")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let miss_reason = systemone_response
            .headers()
            .get("x-aether-local-execution-runtime-miss-reason")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = systemone_response.text().await.unwrap_or_default();
        panic!("systemone decisions expected 200, got {systemone_status}; dependency={dependency_reason:?} miss={miss_reason:?} body={body}");
    }
    assert_eq!(
        systemone_response
            .headers()
            .get(EXECUTION_PATH_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some(EXECUTION_PATH_EXECUTION_RUNTIME_SYNC)
    );
    let systemone_payload: serde_json::Value = systemone_response
        .json()
        .await
        .expect("systemone decisions body should parse");
    assert_eq!(systemone_payload["answers"]["team"]["choice"], "payments");

    // The built upstream plans must match the documented upstream endpoints and
    // forward the request bodies with only the model rewritten to the mapped
    // provider model.
    let plans = seen_plans.lock().expect("mutex should lock").clone();
    let plan_for = |request_id: &str| {
        plans
            .iter()
            .find(|plan| plan["request_id"] == request_id)
            .cloned()
            .unwrap_or_else(|| panic!("plan for {request_id} should be captured"))
    };

    let gemini_plan = plan_for("trace-decisions-gemini-1");
    assert_eq!(
        gemini_plan["url"],
        "https://provider-1.example/v1beta/decisions"
    );
    assert_eq!(gemini_plan["client_api_format"], "gemini:decisions");
    assert_eq!(gemini_plan["provider_api_format"], "gemini:decisions");
    assert_eq!(gemini_plan["stream"], false);
    assert_eq!(
        gemini_plan["headers"]["x-goog-api-key"],
        "sk-upstream-decisions-1"
    );
    let mut expected_gemini = gemini_body.clone();
    expected_gemini["model"] = json!("gemini-decision-1");
    assert_eq!(gemini_plan["body"]["json_body"], expected_gemini);

    let openai_plan = plan_for("trace-decisions-openai-1");
    assert_eq!(
        openai_plan["url"],
        "https://provider-2.example/v1/decisions"
    );
    assert_eq!(openai_plan["client_api_format"], "openai:decisions");
    assert_eq!(openai_plan["provider_api_format"], "openai:decisions");
    assert_eq!(openai_plan["stream"], false);
    assert_eq!(
        openai_plan["headers"]["authorization"],
        "Bearer sk-upstream-decisions-2"
    );
    let mut expected_openai = openai_body.clone();
    expected_openai["model"] = json!("gpt-6-luna");
    assert_eq!(openai_plan["body"]["json_body"], expected_openai);

    let systemone_plan = plan_for("trace-decisions-systemone-1");
    assert_eq!(
        systemone_plan["url"],
        "https://provider-3.example/v1/systemone"
    );
    assert_eq!(systemone_plan["client_api_format"], "systemone:decisions");
    assert_eq!(systemone_plan["provider_api_format"], "systemone:decisions");
    assert_eq!(systemone_plan["stream"], false);
    assert_eq!(
        systemone_plan["headers"]["authorization"],
        "Bearer sk-upstream-decisions-3"
    );
    let mut expected_systemone = systemone_body.clone();
    expected_systemone["model"] = json!("typesafe/jev-1.13");
    assert_eq!(systemone_plan["body"]["json_body"], expected_systemone);

    let candidates = request_candidates
        .list_by_request_id("trace-decisions-gemini-1")
        .await
        .expect("candidates should read");
    assert_eq!(
        candidates
            .iter()
            .filter(|c| c.status == RequestCandidateStatus::Skipped)
            .count(),
        2
    );
    let candidates = candidates
        .into_iter()
        .filter(|c| c.status != RequestCandidateStatus::Skipped)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].status, RequestCandidateStatus::Success);

    gateway_handle.abort();
    execution_runtime_handle.abort();
}
