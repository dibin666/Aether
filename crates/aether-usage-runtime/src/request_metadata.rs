use aether_contracts::ExecutionPlan;
use serde_json::{Map, Value};

pub(crate) fn build_usage_request_metadata_seed(
    plan: &ExecutionPlan,
    context: Option<&Map<String, Value>>,
) -> Option<Value> {
    let mut metadata = context.cloned().unwrap_or_default();
    if !has_non_empty_string(&metadata, "candidate_id") {
        if let Some(candidate_id) = plan
            .candidate_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata.insert(
                "candidate_id".to_string(),
                Value::String(candidate_id.to_string()),
            );
        }
    }
    sanitize_usage_request_metadata(Some(Value::Object(metadata)))
}

pub(crate) fn merge_usage_request_metadata(
    base: Option<Value>,
    override_value: Option<Value>,
) -> Option<Value> {
    let merged = match (base, override_value) {
        (Some(Value::Object(mut base)), Some(Value::Object(override_object))) => {
            for (key, value) in override_object {
                base.insert(key, value);
            }
            Some(Value::Object(base))
        }
        (Some(base), None) => Some(base),
        (_, Some(override_value)) => Some(override_value),
        (None, None) => None,
    };
    sanitize_usage_request_metadata(merged)
}

pub(crate) fn sanitize_usage_request_metadata(value: Option<Value>) -> Option<Value> {
    let Value::Object(object) = value? else {
        return None;
    };

    let mut filtered = Map::new();
    copy_non_empty_string(&object, &mut filtered, "candidate_id");
    copy_number(&object, &mut filtered, "candidate_index");
    copy_non_empty_string(&object, &mut filtered, "key_name");
    copy_non_empty_string(&object, &mut filtered, "trace_id");
    copy_non_null_value(&object, &mut filtered, "billing_snapshot");
    copy_non_null_value(&object, &mut filtered, "dimensions");
    copy_non_null_value(&object, &mut filtered, "billing_rule_snapshot");
    copy_non_null_value(&object, &mut filtered, "scheduling_audit");
    copy_number(&object, &mut filtered, "rate_multiplier");
    copy_bool(&object, &mut filtered, "is_free_tier");

    (!filtered.is_empty()).then_some(Value::Object(filtered))
}

fn has_non_empty_string(object: &Map<String, Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn copy_non_empty_string(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    let Some(value) = source
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    target.insert(key.to_string(), Value::String(value.to_string()));
}

fn copy_number(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    let Some(value) = source.get(key).filter(|value| value.is_number()) else {
        return;
    };
    target.insert(key.to_string(), value.clone());
}

fn copy_bool(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    let Some(value) = source.get(key).filter(|value| value.is_boolean()) else {
        return;
    };
    target.insert(key.to_string(), value.clone());
}

fn copy_non_null_value(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    let Some(value) = source.get(key).filter(|value| !value.is_null()) else {
        return;
    };
    target.insert(key.to_string(), value.clone());
}

#[cfg(test)]
mod tests {
    use aether_contracts::{ExecutionPlan, RequestBody};
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::{
        build_usage_request_metadata_seed, merge_usage_request_metadata,
        sanitize_usage_request_metadata,
    };

    fn sample_plan() -> ExecutionPlan {
        ExecutionPlan {
            request_id: "req-1".to_string(),
            candidate_id: Some("cand-1".to_string()),
            provider_name: Some("OpenAI".to_string()),
            provider_id: "provider-1".to_string(),
            endpoint_id: "endpoint-1".to_string(),
            key_id: "key-1".to_string(),
            method: "POST".to_string(),
            url: "https://example.com/v1/chat/completions".to_string(),
            headers: BTreeMap::new(),
            content_type: None,
            content_encoding: None,
            body: RequestBody::from_json(json!({"model": "gpt-5"})),
            stream: false,
            client_api_format: "openai:chat".to_string(),
            provider_api_format: "openai:chat".to_string(),
            model_name: Some("gpt-5".to_string()),
            proxy: None,
            tls_profile: None,
            timeouts: None,
        }
    }

    #[test]
    fn sanitizes_request_metadata_to_allowlist() {
        let metadata = sanitize_usage_request_metadata(Some(json!({
            "request_id": "req-1",
            "provider_id": "provider-1",
            "provider_name": "OpenAI",
            "model": "gpt-5",
            "candidate_id": "cand-1",
            "candidate_index": 2,
            "key_name": "upstream-primary",
            "trace_id": "trace-1",
            "billing_snapshot": {"status": "complete"},
            "dimensions": {"total_input_context": 10},
            "rate_multiplier": 1.25,
            "is_free_tier": false,
            "original_headers": {"authorization": "Bearer secret"},
            "original_request_body": {"messages": []},
            "provider_request_headers": {"authorization": "Bearer secret"},
            "upstream_url": "https://example.com/v1/chat/completions"
        })))
        .expect("metadata should remain");

        assert_eq!(
            metadata,
            json!({
                "candidate_id": "cand-1",
                "candidate_index": 2,
                "key_name": "upstream-primary",
                "trace_id": "trace-1",
                "billing_snapshot": {"status": "complete"},
                "dimensions": {"total_input_context": 10},
                "rate_multiplier": 1.25,
                "is_free_tier": false
            })
        );
    }

    #[test]
    fn builds_seed_from_context_and_plan_candidate_id() {
        let metadata = build_usage_request_metadata_seed(
            &sample_plan(),
            Some(
                json!({
                    "request_id": "req-1",
                    "candidate_index": 0,
                    "key_name": "upstream-primary",
                    "provider_id": "provider-1",
                    "billing_snapshot": {"status": "complete"}
                })
                .as_object()
                .expect("object"),
            ),
        )
        .expect("metadata should remain");

        assert_eq!(
            metadata,
            json!({
                "candidate_id": "cand-1",
                "candidate_index": 0,
                "key_name": "upstream-primary",
                "billing_snapshot": {"status": "complete"}
            })
        );
    }

    #[test]
    fn merges_and_filters_request_metadata() {
        let metadata = merge_usage_request_metadata(
            Some(json!({
                "candidate_id": "cand-1",
                "request_id": "req-1"
            })),
            Some(json!({
                "candidate_index": 0,
                "key_name": "upstream-primary",
                "provider_name": "OpenAI"
            })),
        )
        .expect("metadata should remain");

        assert_eq!(
            metadata,
            json!({
                "candidate_id": "cand-1",
                "candidate_index": 0,
                "key_name": "upstream-primary"
            })
        );
    }
}
