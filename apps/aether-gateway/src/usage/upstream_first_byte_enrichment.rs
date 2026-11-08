//! Post-hoc first-byte alignment for requests proxied through cli-proxy-api.
//!
//! CPA stops its TTFT clock when the upstream response *headers* arrive, but it
//! does not emit downstream bytes until the first content frame is ready.
//! Requests whose upstream spends a long time in a thinking phase therefore
//! show a much later first byte here than the value CPA reports. When
//! `AETHER_CLIPROXY_MANAGEMENT_KEY` is configured, reports carrying an
//! `X-Cpa-Trace-Id` upstream response header trigger a management-API lookup of
//! the persisted request dump and rewrite the stored first-byte observation
//! with CPA's `request received -> upstream response headers` span.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::task_runtime::spawn_fire_and_forget;
use crate::AppState;

const CPA_TRACE_ID_HEADER: &str = "x-cpa-trace-id";
const CPA_MANAGEMENT_KEY_ENV: &str = "AETHER_CLIPROXY_MANAGEMENT_KEY";
const CPA_REQUEST_LOG_PATH_PREFIX: &str = "/v0/management/request-log-by-id/";
const CPA_TRACE_ID_REQUEST_ID_LEN: usize = 36;
const FETCH_ATTEMPTS: usize = 8;
const UPDATE_ATTEMPTS: usize = 10;
const TASK_NAME: &str = "usage.upstream_first_byte.enrichment";

/// Spawn a background enrichment for a report that travelled through CPA.
///
/// Cheap no-op unless the upstream response carried `X-Cpa-Trace-Id`, the
/// report context names a resolvable upstream origin, and the management key
/// is configured.
pub(crate) fn spawn_upstream_first_byte_enrichment(
    state: &AppState,
    report_context: Option<&Value>,
    measured_first_byte_time_ms: Option<u64>,
) {
    let Some(management_key) = configured_management_key() else {
        return;
    };
    let Some(plan) =
        EnrichmentPlan::from_report_context(report_context, measured_first_byte_time_ms)
    else {
        return;
    };
    let state = state.clone();
    spawn_fire_and_forget(TASK_NAME, async move {
        if let Err(error) = plan.run(&state, &management_key).await {
            debug!(
                event_name = "usage_first_byte_enrichment_skipped",
                log_type = "debug",
                debug_context = "redacted",
                request_id = %plan.request_id,
                error = %error,
                "skipped upstream first-byte enrichment"
            );
        }
    });
}

fn configured_management_key() -> Option<String> {
    std::env::var(CPA_MANAGEMENT_KEY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug)]
struct EnrichmentPlan {
    request_id: String,
    lookup_url: String,
    cpa_request_id: String,
    provider_request_started_at_unix_ms: Option<u64>,
    measured_first_byte_time_ms: Option<u64>,
    measured_e2e_first_byte_time_ms: Option<u64>,
    e2e_time_ms: Option<u64>,
}

impl EnrichmentPlan {
    fn from_report_context(
        report_context: Option<&Value>,
        measured_first_byte_time_ms: Option<u64>,
    ) -> Option<Self> {
        let context = report_context?.as_object()?;
        let request_id = context
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?
            .to_string();
        let trace_id = context
            .get("provider_response_headers")
            .and_then(Value::as_object)
            .and_then(|headers| {
                headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(CPA_TRACE_ID_HEADER))
                    .and_then(|(_, value)| value.as_str())
            })
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let cpa_request_id = cpa_request_id_from_trace_id(trace_id)?;
        let upstream_url = context.get("upstream_url").and_then(Value::as_str)?;
        let lookup_url = cpa_request_log_url(upstream_url, &cpa_request_id)?;

        Some(Self {
            request_id,
            lookup_url,
            cpa_request_id,
            provider_request_started_at_unix_ms: context
                .get("provider_request_started_at_unix_ms")
                .and_then(Value::as_u64),
            measured_first_byte_time_ms,
            measured_e2e_first_byte_time_ms: context
                .get("end_to_end_first_byte_time_ms")
                .and_then(Value::as_u64),
            e2e_time_ms: context.get("end_to_end_time_ms").and_then(Value::as_u64),
        })
    }

    async fn run(&self, state: &AppState, management_key: &str) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|error| format!("build enrichment http client: {error}"))?;

        let mut observation = None;
        for attempt in 0..FETCH_ATTEMPTS {
            sleep(Duration::from_millis(if attempt == 0 {
                400
            } else {
                1_500
            }))
            .await;
            match fetch_cpa_request_log(&client, &self.lookup_url, management_key).await {
                FetchOutcome::Body(body) => {
                    if let Some((received_ms, upstream_ms)) = parse_cpa_log_timestamps(&body) {
                        observation = Some((received_ms, upstream_ms));
                        break;
                    }
                }
                FetchOutcome::Retry => {}
                FetchOutcome::Abort(error) => return Err(error),
            }
        }
        let Some((cpa_received_ms, cpa_upstream_response_ms)) = observation else {
            return Err("cpa request log was not readable in time".to_string());
        };
        let Some((aligned_first_byte_ms, aligned_e2e_ms, cpa_ttft_ms)) = aligned_first_byte_metrics(
            cpa_received_ms,
            cpa_upstream_response_ms,
            self.provider_request_started_at_unix_ms,
            self.measured_first_byte_time_ms,
            self.measured_e2e_first_byte_time_ms,
            self.e2e_time_ms,
        ) else {
            return Err("cpa timestamps were not ordered".to_string());
        };

        let mut patch = json!({
            "first_byte_time_ms_source": "cliproxy",
            "cliproxy_request_id": self.cpa_request_id.as_str(),
            "cliproxy_ttft_ms": cpa_ttft_ms.max(0),
        });
        if let Some(measured) = self.measured_first_byte_time_ms {
            patch["measured_first_byte_time_ms"] = json!(measured);
        }
        if let Some(measured_e2e) = self.measured_e2e_first_byte_time_ms {
            patch["measured_end_to_end_first_byte_time_ms"] = json!(measured_e2e);
        }
        if let Some(aligned_e2e) = aligned_e2e_ms {
            patch["end_to_end_first_byte_time_ms"] = json!(aligned_e2e);
        }

        for attempt in 0..UPDATE_ATTEMPTS {
            if attempt > 0 {
                sleep(Duration::from_millis(1_000)).await;
            }
            match state
                .data
                .update_usage_first_byte_time_ms(
                    &self.request_id,
                    aligned_first_byte_ms,
                    Some(&patch),
                )
                .await
            {
                Ok(true) => {
                    debug!(
                        event_name = "usage_first_byte_enrichment_applied",
                        log_type = "debug",
                        debug_context = "redacted",
                        request_id = %self.request_id,
                        cliproxy_ttft_ms = cpa_ttft_ms,
                        aligned_first_byte_time_ms = aligned_first_byte_ms,
                        "aligned usage first-byte observation to upstream proxy timing"
                    );
                    return Ok(());
                }
                Ok(false) => continue,
                Err(error) => {
                    warn!(
                        event_name = "usage_first_byte_enrichment_failed",
                        log_type = "ops",
                        request_id = %self.request_id,
                        error = %error,
                        "failed to persist upstream first-byte enrichment"
                    );
                    return Err(error.to_string());
                }
            }
        }
        Err("usage row never reached a terminal status".to_string())
    }
}

/// CPA trace ids look like `20261108134358-f7971dee06bc10a8-<36-char ulid>`;
/// the management lookup accepts the trailing request ULID.
fn cpa_request_id_from_trace_id(trace_id: &str) -> Option<String> {
    let trace_id = trace_id.trim();
    if trace_id.len() <= CPA_TRACE_ID_REQUEST_ID_LEN {
        return None;
    }
    let candidate = &trace_id[trace_id.len() - CPA_TRACE_ID_REQUEST_ID_LEN..];
    let separator = trace_id.as_bytes()[trace_id.len() - CPA_TRACE_ID_REQUEST_ID_LEN - 1];
    if separator != b'-'
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return None;
    }
    Some(candidate.to_string())
}

fn cpa_request_log_url(upstream_url: &str, cpa_request_id: &str) -> Option<String> {
    let parsed = url::Url::parse(upstream_url).ok()?;
    let origin = parsed.origin().ascii_serialization();
    if !origin.starts_with("http") {
        return None;
    }
    Some(format!(
        "{origin}{CPA_REQUEST_LOG_PATH_PREFIX}{cpa_request_id}"
    ))
}

enum FetchOutcome {
    Body(String),
    Retry,
    Abort(String),
}

async fn fetch_cpa_request_log(
    client: &reqwest::Client,
    lookup_url: &str,
    management_key: &str,
) -> FetchOutcome {
    let response = match client
        .get(lookup_url)
        .bearer_auth(management_key)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return FetchOutcome::Retry,
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        // CPA persists the dump when the request finishes; it can lag behind
        // our own finalization by a few hundred milliseconds.
        return FetchOutcome::Retry;
    }
    if !response.status().is_success() {
        return FetchOutcome::Abort(format!(
            "cpa request log lookup failed with status {}",
            response.status()
        ));
    }
    match response.text().await {
        Ok(body) => FetchOutcome::Body(body),
        Err(_) => FetchOutcome::Retry,
    }
}

/// Extract `received -> upstream response headers` from a CPA request dump.
///
/// The dump starts with `=== REQUEST INFO ===` (its `Timestamp:` is when CPA
/// accepted the request) followed by `=== API RESPONSE 1 ===` whose timestamp
/// marks the upstream response headers. Both are RFC3339 with nanoseconds.
fn parse_cpa_log_timestamps(body: &str) -> Option<(i64, i64)> {
    let received = timestamp_after_marker(body, "=== REQUEST INFO ===")?;
    let upstream_response = timestamp_after_marker(body, "=== API RESPONSE 1 ===")?;
    (upstream_response >= received).then_some((received, upstream_response))
}

fn timestamp_after_marker(body: &str, marker: &str) -> Option<i64> {
    let section = &body[body.find(marker)?..];
    let prefix = "Timestamp:";
    let start = section.find(prefix)? + prefix.len();
    let line_end = section[start..]
        .find('\n')
        .map(|offset| start + offset)
        .unwrap_or(section.len());
    let raw = section[start..line_end].trim();
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|value| value.timestamp_millis())
}

/// Map the CPA-observed span onto this request's usage timeline.
///
/// Returns `(aligned_first_byte_ms, aligned_e2e_first_byte_ms, cpa_ttft_ms)`.
/// The candidate-level value keeps the real transit time between our upstream
/// dispatch and CPA's accept, then swaps CPA's head-wait for the buffered
/// downstream wait. The end-to-end value preserves the pre-candidate offset
/// (queueing, retries) measured by the gateway.
fn aligned_first_byte_metrics(
    cpa_received_ms: i64,
    cpa_upstream_response_ms: i64,
    provider_request_started_at_unix_ms: Option<u64>,
    measured_first_byte_ms: Option<u64>,
    measured_e2e_first_byte_ms: Option<u64>,
    e2e_time_ms: Option<u64>,
) -> Option<(u64, Option<u64>, i64)> {
    let cpa_ttft_ms = cpa_upstream_response_ms.checked_sub(cpa_received_ms)?;
    if cpa_ttft_ms < 0 {
        return None;
    }
    let transit_ms = provider_request_started_at_unix_ms
        .and_then(|started| cpa_received_ms.checked_sub(started as i64))
        .unwrap_or(0)
        .max(0);
    let aligned = (cpa_ttft_ms + transit_ms).max(0) as u64;
    let aligned_e2e = match (measured_e2e_first_byte_ms, measured_first_byte_ms) {
        (Some(e2e), Some(first_byte)) => {
            let value = e2e.saturating_sub(first_byte).saturating_add(aligned);
            Some(e2e_time_ms.map(|cap| value.min(cap)).unwrap_or(value))
        }
        _ => None,
    };
    Some((aligned, aligned_e2e, cpa_ttft_ms))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        aligned_first_byte_metrics, cpa_request_id_from_trace_id, cpa_request_log_url,
        parse_cpa_log_timestamps, EnrichmentPlan,
    };

    #[test]
    fn extracts_request_id_from_cpa_trace_id() {
        assert_eq!(
            cpa_request_id_from_trace_id(
                "20261108134358-f7971dee06bc10a8-01a1b9af-57f2-7778-b00e-9adb933d8410"
            )
            .as_deref(),
            Some("01a1b9af-57f2-7778-b00e-9adb933d8410")
        );
        assert!(cpa_request_id_from_trace_id("short").is_none());
        assert!(cpa_request_id_from_trace_id("x".repeat(40).as_str()).is_none());
    }

    #[test]
    fn builds_management_lookup_url_from_upstream_origin() {
        assert_eq!(
            cpa_request_log_url("http://cli-proxy-api:8317/v1/messages", "req-1").as_deref(),
            Some("http://cli-proxy-api:8317/v0/management/request-log-by-id/req-1")
        );
        assert!(cpa_request_log_url("not a url", "req-1").is_none());
    }

    #[test]
    fn parses_request_and_upstream_response_timestamps() {
        let body = concat!(
            "=== REQUEST INFO ===\n",
            "Version: v8.0.59\n",
            "Timestamp: 2026-11-08T13:43:58.706500776+08:00\n",
            "\n=== HEADERS ===\n",
            "X-Api-Key: kDRX\n",
            "\n=== API REQUEST ===\n",
            "Timestamp: 2026-11-08T13:43:58.754128451+08:00\n",
            "\n=== API RESPONSE 1 ===\n",
            "Timestamp: 2026-11-08T13:44:02.503460193+08:00\n",
        );
        let (received, upstream) = parse_cpa_log_timestamps(body).expect("timestamps");
        assert_eq!(upstream - received, 3_797);
    }

    #[test]
    fn aligns_candidate_and_end_to_end_first_byte() {
        // received=100_000, upstream response=103_797 -> cpa ttft 3_797.
        // provider started at 99_950 -> transit 50ms; measured candidate fb
        // 9_091, measured e2e fb 9_457, e2e total 16_112.
        let (aligned, aligned_e2e, ttft) = aligned_first_byte_metrics(
            100_000,
            103_797,
            Some(99_950),
            Some(9_091),
            Some(9_457),
            Some(16_112),
        )
        .expect("aligned metrics");
        assert_eq!(ttft, 3_797);
        assert_eq!(aligned, 3_847);
        assert_eq!(aligned_e2e, Some(4_213));
    }

    #[test]
    fn plan_requires_trace_header_and_upstream_url() {
        assert!(
            EnrichmentPlan::from_report_context(Some(&json!({"request_id": "r1"})), None).is_none()
        );
        let plan = EnrichmentPlan::from_report_context(
            Some(&json!({
                "request_id": "req-1",
                "upstream_url": "http://cli-proxy-api:8317/v1/messages",
                "provider_response_headers": {
                    "x-cpa-trace-id": "20261108134358-f7971dee06bc10a8-01a1b9af-57f2-7778-b00e-9adb933d8410"
                }
            })),
            Some(9_091),
        )
        .expect("plan should build");
        assert_eq!(plan.request_id, "req-1");
        assert_eq!(plan.measured_first_byte_time_ms, Some(9_091));
    }
}
