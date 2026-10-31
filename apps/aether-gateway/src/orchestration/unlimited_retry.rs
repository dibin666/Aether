use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use aether_ai_serving::{AiAttemptRetryScope, AiExecutionAttempt};
use serde_json::Value;
use tracing::info;
use uuid::Uuid;

use super::attempt::{
    attempt_identity_from_report_context, next_same_key_retry_attempt, ExecutionAttemptIdentity,
    POOL_KEY_RETRY_INDEX_STRIDE,
};
use crate::AppState;

pub(crate) const UNLIMITED_RETRY_CONFIG_KEY: &str = "unlimited_retry";

const MAX_UNLIMITED_RETRY_AFTER: Duration = Duration::from_secs(60);
const MAX_UPSTREAM_RETRY_HINTS: usize = 4096;

/// Upstream `Retry-After` hints keyed by the failed attempt's candidate id.
/// Entries are consumed by the same-key retry that follows the failure.
static UPSTREAM_RETRY_HINTS: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn provider_unlimited_retry_enabled(config: Option<&Value>) -> bool {
    config
        .and_then(|config| config.get(UNLIMITED_RETRY_CONFIG_KEY))
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn parse_upstream_retry_after(headers: &BTreeMap<String, String>) -> Option<Duration> {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim())
    };
    if let Some(milliseconds) = header("retry-after-ms").and_then(|value| value.parse::<f64>().ok())
    {
        if let Some(delay) = bounded_duration_from_secs(milliseconds / 1000.0) {
            return Some(delay);
        }
    }
    let value = header("retry-after")?;
    if let Ok(seconds) = value.parse::<f64>() {
        return bounded_duration_from_secs(seconds);
    }
    let retry_at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let delay = retry_at
        .with_timezone(&chrono::Utc)
        .signed_duration_since(chrono::Utc::now())
        .to_std()
        .unwrap_or(Duration::ZERO);
    Some(delay.min(MAX_UNLIMITED_RETRY_AFTER))
}

fn bounded_duration_from_secs(seconds: f64) -> Option<Duration> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(
        seconds.min(MAX_UNLIMITED_RETRY_AFTER.as_secs_f64()),
    ))
}

pub(crate) fn record_upstream_retry_hint(candidate_id: &str, retry_after: Duration) {
    let now = Instant::now();
    let mut hints = UPSTREAM_RETRY_HINTS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if hints.len() >= MAX_UPSTREAM_RETRY_HINTS {
        hints.retain(|_, not_before| *not_before > now);
        if hints.len() >= MAX_UPSTREAM_RETRY_HINTS {
            hints.clear();
        }
    }
    hints.insert(candidate_id.to_string(), now + retry_after);
}

fn take_upstream_retry_hint(candidate_id: &str) -> Option<Instant> {
    UPSTREAM_RETRY_HINTS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(candidate_id)
}

/// Retry index of the next unlimited same-key attempt, or `None` when the
/// current pool key has no retry index left.
///
/// Unlike `next_same_key_retry_index`, every candidate may retry and the
/// sticky-key budget does not apply. Pool keys keep their encoded retry index
/// inside their own stride, so each pool key gets at most
/// `POOL_KEY_RETRY_INDEX_STRIDE - 1` same-key retries; after that the normal
/// failover continues to the next key.
pub(crate) fn next_unlimited_same_key_retry_index(
    identity: ExecutionAttemptIdentity,
) -> Option<u32> {
    let next = identity.retry_index.checked_add(1)?;
    match identity.pool_key_index {
        None => i32::try_from(next).ok().map(|_| next),
        Some(pool_key_index) => {
            let limit = pool_key_index
                .checked_add(1)?
                .checked_mul(POOL_KEY_RETRY_INDEX_STRIDE)?;
            (next < limit).then_some(next)
        }
    }
}

/// Derive the next same-key attempt after a failure. Providers without
/// unlimited retry keep the sticky-key behavior. With unlimited retry enabled,
/// any scope retries on the same key until the client disconnects, the
/// retry index space runs out, or a stop rule responds. An upstream
/// `Retry-After` hint delays the retry.
pub(crate) async fn resolve_next_same_key_retry_attempt<A: AiExecutionAttempt + Send + Sync>(
    state: &AppState,
    attempt: &A,
    scope: AiAttemptRetryScope,
) -> Option<A> {
    let plan = attempt.execution_plan();
    let unlimited = match state
        .read_provider_transport_snapshot(&plan.provider_id, &plan.endpoint_id, &plan.key_id)
        .await
    {
        Ok(Some(snapshot)) => provider_unlimited_retry_enabled(snapshot.provider.config.as_ref()),
        _ => false,
    };
    if !unlimited {
        return if scope == AiAttemptRetryScope::Candidate {
            next_same_key_retry_attempt(attempt)
        } else {
            None
        };
    }
    if crate::request_lifecycle::client_disconnected() {
        return None;
    }

    let owned_report_context = attempt
        .report_context_ref()
        .is_none()
        .then(|| attempt.report_context())
        .flatten();
    let report_context = attempt
        .report_context_ref()
        .or(owned_report_context.as_ref());
    let identity = attempt_identity_from_report_context(report_context)?;
    let retry_index = next_unlimited_same_key_retry_index(identity)?;

    let not_before = plan
        .candidate_id
        .as_deref()
        .and_then(take_upstream_retry_hint);
    let delay = not_before
        .map(|not_before| not_before.saturating_duration_since(Instant::now()))
        .unwrap_or_default();
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
        if crate::request_lifecycle::client_disconnected() {
            return None;
        }
    }

    info!(
        event_name = "unlimited_same_key_retry",
        request_id = %plan.request_id,
        provider_id = %plan.provider_id,
        key_id = %plan.key_id,
        retry_index,
        scope = ?scope,
        delay_ms = delay.as_millis() as u64,
        "retrying failed upstream attempt on the same key"
    );
    attempt.with_same_key_retry(retry_index, Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use serde_json::json;

    use super::{
        bounded_duration_from_secs, next_unlimited_same_key_retry_index,
        parse_upstream_retry_after, provider_unlimited_retry_enabled, record_upstream_retry_hint,
        take_upstream_retry_hint, ExecutionAttemptIdentity,
    };

    fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn unlimited_index_ignores_candidate_index_and_advances_without_budget() {
        assert_eq!(
            next_unlimited_same_key_retry_index(ExecutionAttemptIdentity::new(3, 500)),
            Some(501)
        );
    }

    #[test]
    fn unlimited_index_stops_at_pool_key_stride_boundary() {
        let first_pool_key = ExecutionAttemptIdentity::new(0, 98).with_pool_key_index(Some(0));
        assert_eq!(
            next_unlimited_same_key_retry_index(first_pool_key),
            Some(99)
        );
        let last = ExecutionAttemptIdentity::new(0, 99).with_pool_key_index(Some(0));
        assert_eq!(next_unlimited_same_key_retry_index(last), None);

        let third_pool_key = ExecutionAttemptIdentity::new(0, 298).with_pool_key_index(Some(2));
        assert_eq!(
            next_unlimited_same_key_retry_index(third_pool_key),
            Some(299)
        );
        let third_last = ExecutionAttemptIdentity::new(0, 299).with_pool_key_index(Some(2));
        assert_eq!(next_unlimited_same_key_retry_index(third_last), None);
    }

    #[test]
    fn unlimited_index_fails_closed_when_it_would_overflow_database_column() {
        assert_eq!(
            next_unlimited_same_key_retry_index(ExecutionAttemptIdentity::new(0, i32::MAX as u32)),
            None
        );
        assert_eq!(
            next_unlimited_same_key_retry_index(ExecutionAttemptIdentity::new(0, u32::MAX)),
            None
        );
    }

    #[test]
    fn retry_after_accepts_seconds_and_fractional_seconds() {
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("retry-after", "3")])),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("retry-after", "1.5")])),
            Some(Duration::from_millis(1500))
        );
    }

    #[test]
    fn retry_after_ms_takes_precedence_over_seconds() {
        assert_eq!(
            parse_upstream_retry_after(&headers(&[
                ("retry-after", "3"),
                ("retry-after-ms", "250"),
            ])),
            Some(Duration::from_millis(250))
        );
    }

    #[test]
    fn retry_after_header_names_are_case_insensitive() {
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("Retry-After", "2")])),
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn retry_after_http_date_in_future_yields_remaining_delay() {
        let retry_at = (chrono::Utc::now() + chrono::TimeDelta::seconds(10)).to_rfc2822();
        let delay = parse_upstream_retry_after(&headers(&[("retry-after", retry_at.as_str())]))
            .expect("http date should parse");
        assert!(delay > Duration::from_secs(8), "delay was {delay:?}");
        assert!(delay <= Duration::from_secs(10), "delay was {delay:?}");
    }

    #[test]
    fn retry_after_is_capped_and_garbage_is_ignored() {
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("retry-after", "999")])),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("retry-after", "soon")])),
            None
        );
        assert_eq!(
            parse_upstream_retry_after(&headers(&[("retry-after", "-5")])),
            None
        );
        assert_eq!(bounded_duration_from_secs(f64::NAN), None);
        assert_eq!(parse_upstream_retry_after(&headers(&[])), None);
    }

    #[test]
    fn upstream_retry_hint_is_taken_once() {
        record_upstream_retry_hint("candidate-hint-round-trip", Duration::from_secs(5));
        assert!(take_upstream_retry_hint("candidate-hint-round-trip").is_some());
        assert!(take_upstream_retry_hint("candidate-hint-round-trip").is_none());
    }

    #[test]
    fn unlimited_retry_flag_defaults_to_disabled() {
        assert!(provider_unlimited_retry_enabled(Some(
            &json!({"unlimited_retry": {"enabled": true}})
        )));
        assert!(!provider_unlimited_retry_enabled(Some(
            &json!({"unlimited_retry": {"enabled": false}})
        )));
        assert!(!provider_unlimited_retry_enabled(Some(
            &json!({"unlimited_retry": {"enabled": "yes"}})
        )));
        assert!(!provider_unlimited_retry_enabled(None));
    }
}
