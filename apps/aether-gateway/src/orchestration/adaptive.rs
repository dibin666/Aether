use std::collections::BTreeMap;

use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use serde_json::{json, Value};

use super::LocalFailoverClassification;
use crate::handlers::shared::default_provider_key_status_snapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalAdaptiveRateLimitProjection {
    pub(crate) rpm_429_count: u32,
    pub(crate) last_429_at_unix_secs: u64,
    pub(crate) last_429_type: String,
    pub(crate) status_snapshot: Value,
}

pub(crate) fn project_local_adaptive_rate_limit(
    current_key: &StoredProviderCatalogKey,
    classification: LocalFailoverClassification,
    status_code: u16,
    headers: Option<&BTreeMap<String, String>>,
    observed_at_unix_secs: u64,
) -> Option<LocalAdaptiveRateLimitProjection> {
    if current_key.rpm_limit.is_some() {
        return None;
    }

    if !local_candidate_failure_should_record_adaptive_rate_limit(classification, status_code) {
        return None;
    }

    let latest_upstream_limit = parse_latest_upstream_limit(headers);
    Some(LocalAdaptiveRateLimitProjection {
        rpm_429_count: current_key
            .rpm_429_count
            .unwrap_or_default()
            .saturating_add(1),
        last_429_at_unix_secs: observed_at_unix_secs,
        last_429_type: "rpm".to_string(),
        status_snapshot: project_local_adaptive_status_snapshot(current_key, latest_upstream_limit),
    })
}

fn local_candidate_failure_should_record_adaptive_rate_limit(
    classification: LocalFailoverClassification,
    status_code: u16,
) -> bool {
    status_code == 429
        || matches!(
            classification,
            LocalFailoverClassification::RetrySemanticRateLimit
        )
}

fn project_local_adaptive_status_snapshot(
    current_key: &StoredProviderCatalogKey,
    latest_upstream_limit: Option<u64>,
) -> Value {
    let default_snapshot = default_provider_key_status_snapshot();
    let mut snapshot = current_key
        .status_snapshot
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| default_snapshot.as_object().cloned())
        .unwrap_or_default();

    let observation_count = snapshot
        .get("observation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    snapshot.insert("observation_count".to_string(), json!(observation_count));

    if let Some(limit) = latest_upstream_limit {
        let header_observation_count = snapshot
            .get("header_observation_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        snapshot.insert(
            "header_observation_count".to_string(),
            json!(header_observation_count),
        );
        snapshot.insert("latest_upstream_limit".to_string(), json!(limit));
    }

    let header_observation_count = snapshot
        .get("header_observation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let effective_upstream_limit = latest_upstream_limit.or_else(|| {
        snapshot
            .get("latest_upstream_limit")
            .and_then(Value::as_u64)
    });
    let learning_confidence = projected_learning_confidence(
        observation_count,
        header_observation_count,
        current_key.learned_rpm_limit.is_some(),
        effective_upstream_limit.is_some(),
    );
    snapshot.insert(
        "learning_confidence".to_string(),
        json!(learning_confidence),
    );
    snapshot.insert(
        "enforcement_active".to_string(),
        json!(adaptive_enforcement_active(
            learning_confidence,
            current_key.learned_rpm_limit.is_some(),
            effective_upstream_limit.is_some(),
        )),
    );

    Value::Object(snapshot)
}

fn projected_learning_confidence(
    observation_count: u64,
    header_observation_count: u64,
    has_learned_limit: bool,
    has_upstream_limit: bool,
) -> f64 {
    let base = if has_learned_limit || has_upstream_limit {
        0.1
    } else {
        0.0
    };
    let observation_score = (observation_count.min(8) as f64) * 0.05;
    let header_score = (header_observation_count.min(3) as f64) * (0.4 / 3.0);
    ((base + observation_score + header_score).min(1.0) * 1000.0).round() / 1000.0
}

fn adaptive_enforcement_active(
    learning_confidence: f64,
    has_learned_limit: bool,
    has_upstream_limit: bool,
) -> bool {
    (has_learned_limit || has_upstream_limit) && learning_confidence >= 0.5
}

fn parse_latest_upstream_limit(headers: Option<&BTreeMap<String, String>>) -> Option<u64> {
    let normalized = headers?
        .iter()
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<BTreeMap<_, _>>();

    const CANDIDATE_KEYS: &[&str] = &[
        "x-ratelimit-limit-requests",
        "x-ratelimit-limit-request",
        "x-ratelimit-limit",
        "x-rate-limit-limit",
        "ratelimit-limit",
    ];

    for key in CANDIDATE_KEYS {
        if let Some(limit) = normalized
            .get(*key)
            .and_then(|value| parse_limit_header_value(value))
        {
            return Some(limit);
        }
    }

    normalized.iter().find_map(|(key, value)| {
        if !key.contains("ratelimit") || !key.contains("limit") {
            return None;
        }
        if key.contains("token") {
            return None;
        }
        parse_limit_header_value(value)
    })
}

fn parse_limit_header_value(raw: &str) -> Option<u64> {
    raw.split([',', ';'])
        .find_map(|part| {
            let digits = part
                .trim()
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            (!digits.is_empty())
                .then(|| digits.parse::<u64>().ok())
                .flatten()
        })
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::project_local_adaptive_rate_limit;
    use crate::orchestration::LocalFailoverClassification;
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
    use serde_json::json;

    fn sample_adaptive_key() -> StoredProviderCatalogKey {
        let mut key = StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "provider-1".to_string(),
            "adaptive".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build");
        key.rpm_limit = None;
        key.learned_rpm_limit = Some(12);
        key.rpm_429_count = Some(2);
        key
    }

    #[test]
    fn rate_limit_projection_increments_adaptive_rpm_observation() {
        let key = sample_adaptive_key();

        let projection = project_local_adaptive_rate_limit(
            &key,
            LocalFailoverClassification::RetrySemanticRateLimit,
            429,
            None,
            1_760_000_000,
        )
        .expect("projection should exist");

        assert_eq!(projection.rpm_429_count, 3);
        assert_eq!(projection.last_429_at_unix_secs, 1_760_000_000);
        assert_eq!(projection.last_429_type, "rpm");
        assert_eq!(projection.status_snapshot["observation_count"], json!(1));
        assert_eq!(
            projection.status_snapshot["learning_confidence"],
            json!(0.15)
        );
        assert_eq!(
            projection.status_snapshot["enforcement_active"],
            json!(false)
        );
    }

    #[test]
    fn rate_limit_projection_ignores_fixed_limit_keys() {
        let mut key = sample_adaptive_key();
        key.rpm_limit = Some(20);

        assert!(project_local_adaptive_rate_limit(
            &key,
            LocalFailoverClassification::RetrySemanticRateLimit,
            429,
            None,
            1_760_000_000,
        )
        .is_none());
    }

    #[test]
    fn rate_limit_projection_ignores_non_rate_limit_failures() {
        let key = sample_adaptive_key();

        assert!(project_local_adaptive_rate_limit(
            &key,
            LocalFailoverClassification::RetryUpstreamFailure,
            503,
            None,
            1_760_000_000,
        )
        .is_none());
    }

    #[test]
    fn rate_limit_projection_records_header_observation_and_limit() {
        let mut key = sample_adaptive_key();
        key.status_snapshot = Some(json!({
            "oauth": { "code": "ok" },
            "observation_count": 4,
            "header_observation_count": 1,
            "latest_upstream_limit": 20
        }));
        let headers =
            BTreeMap::from([("x-ratelimit-limit-requests".to_string(), "60".to_string())]);

        let projection = project_local_adaptive_rate_limit(
            &key,
            LocalFailoverClassification::RetrySemanticRateLimit,
            429,
            Some(&headers),
            1_760_000_000,
        )
        .expect("projection should exist");

        assert_eq!(projection.status_snapshot["observation_count"], json!(5));
        assert_eq!(
            projection.status_snapshot["header_observation_count"],
            json!(2)
        );
        assert_eq!(
            projection.status_snapshot["latest_upstream_limit"],
            json!(60)
        );
        assert_eq!(projection.status_snapshot["oauth"]["code"], json!("ok"));
    }

    #[test]
    fn rate_limit_projection_derives_confidence_and_enforcement_from_evidence() {
        let mut key = sample_adaptive_key();
        key.status_snapshot = Some(json!({
            "observation_count": 7,
            "header_observation_count": 2,
            "latest_upstream_limit": 24
        }));
        let headers =
            BTreeMap::from([("x-ratelimit-limit-requests".to_string(), "60".to_string())]);

        let projection = project_local_adaptive_rate_limit(
            &key,
            LocalFailoverClassification::RetrySemanticRateLimit,
            429,
            Some(&headers),
            1_760_000_000,
        )
        .expect("projection should exist");

        assert_eq!(
            projection.status_snapshot["learning_confidence"],
            json!(0.9)
        );
        assert_eq!(
            projection.status_snapshot["enforcement_active"],
            json!(true)
        );
    }
}
