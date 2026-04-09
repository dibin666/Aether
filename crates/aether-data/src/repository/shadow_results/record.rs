use super::types::{ShadowResultMatchStatus, StoredShadowResult, UpsertShadowResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowResultSampleOrigin {
    Rust,
    Python,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordShadowResultSample {
    pub trace_id: String,
    pub request_fingerprint: String,
    pub request_id: Option<String>,
    pub route_family: Option<String>,
    pub route_kind: Option<String>,
    pub candidate_id: Option<String>,
    pub origin: ShadowResultSampleOrigin,
    pub result_digest: String,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub recorded_at_unix_secs: u64,
}

pub fn merge_shadow_result_sample(
    existing: Option<&StoredShadowResult>,
    sample: RecordShadowResultSample,
) -> UpsertShadowResult {
    let RecordShadowResultSample {
        trace_id,
        request_fingerprint,
        request_id,
        route_family,
        route_kind,
        candidate_id,
        origin,
        result_digest,
        status_code,
        error_message,
        recorded_at_unix_secs,
    } = sample;

    let (rust_result_digest, python_result_digest) = match origin {
        ShadowResultSampleOrigin::Rust => (
            Some(result_digest),
            existing.and_then(|row| row.python_result_digest.clone()),
        ),
        ShadowResultSampleOrigin::Python => (
            existing.and_then(|row| row.rust_result_digest.clone()),
            Some(result_digest),
        ),
    };

    let match_status = resolve_match_status(
        rust_result_digest.as_deref(),
        python_result_digest.as_deref(),
    );

    UpsertShadowResult {
        trace_id,
        request_fingerprint,
        request_id: request_id.or_else(|| existing.and_then(|row| row.request_id.clone())),
        route_family: route_family.or_else(|| existing.and_then(|row| row.route_family.clone())),
        route_kind: route_kind.or_else(|| existing.and_then(|row| row.route_kind.clone())),
        candidate_id: candidate_id.or_else(|| existing.and_then(|row| row.candidate_id.clone())),
        rust_result_digest,
        python_result_digest,
        match_status,
        status_code: status_code.or(existing.and_then(|row| row.status_code)),
        error_message: resolve_error_message(existing, error_message, match_status),
        created_at_unix_ms: existing
            .map(|row| row.created_at_unix_ms)
            .unwrap_or(recorded_at_unix_secs),
        updated_at_unix_secs: recorded_at_unix_secs,
    }
}

fn resolve_match_status(
    rust_result_digest: Option<&str>,
    python_result_digest: Option<&str>,
) -> ShadowResultMatchStatus {
    match (rust_result_digest, python_result_digest) {
        (Some(rust_digest), Some(python_digest)) if rust_digest == python_digest => {
            ShadowResultMatchStatus::Match
        }
        (Some(_), Some(_)) => ShadowResultMatchStatus::Mismatch,
        _ => ShadowResultMatchStatus::Pending,
    }
}

fn resolve_error_message(
    existing: Option<&StoredShadowResult>,
    error_message: Option<String>,
    match_status: ShadowResultMatchStatus,
) -> Option<String> {
    if match_status == ShadowResultMatchStatus::Mismatch {
        error_message
            .or_else(|| existing.and_then(|row| row.error_message.clone()))
            .or_else(|| Some("shadow result digest mismatch".to_string()))
    } else {
        error_message.or_else(|| existing.and_then(|row| row.error_message.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_shadow_result_sample, RecordShadowResultSample, ShadowResultSampleOrigin};
    use crate::repository::shadow_results::{ShadowResultMatchStatus, UpsertShadowResult};

    fn rust_sample(result_digest: &str, recorded_at_unix_secs: u64) -> RecordShadowResultSample {
        RecordShadowResultSample {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            origin: ShadowResultSampleOrigin::Rust,
            result_digest: result_digest.to_string(),
            status_code: Some(200),
            error_message: None,
            recorded_at_unix_secs,
        }
    }

    fn python_sample(result_digest: &str, recorded_at_unix_secs: u64) -> RecordShadowResultSample {
        RecordShadowResultSample {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: None,
            origin: ShadowResultSampleOrigin::Python,
            result_digest: result_digest.to_string(),
            status_code: Some(200),
            error_message: None,
            recorded_at_unix_secs,
        }
    }

    fn stored(upsert: UpsertShadowResult) -> crate::repository::shadow_results::StoredShadowResult {
        upsert.into_stored()
    }

    #[test]
    fn keeps_pending_until_both_samples_exist() {
        let merged = merge_shadow_result_sample(None, rust_sample("digest-1", 100));

        assert_eq!(merged.match_status, ShadowResultMatchStatus::Pending);
        assert_eq!(merged.request_id.as_deref(), Some("req-1"));
        assert_eq!(merged.rust_result_digest.as_deref(), Some("digest-1"));
        assert!(merged.python_result_digest.is_none());
    }

    #[test]
    fn marks_match_when_rust_and_python_digests_are_equal() {
        let existing = stored(merge_shadow_result_sample(
            None,
            rust_sample("digest-1", 100),
        ));
        let merged = merge_shadow_result_sample(Some(&existing), python_sample("digest-1", 200));

        assert_eq!(merged.match_status, ShadowResultMatchStatus::Match);
        assert_eq!(merged.created_at_unix_ms, 100);
        assert_eq!(merged.updated_at_unix_secs, 200);
        assert_eq!(merged.request_id.as_deref(), Some("req-1"));
        assert_eq!(merged.rust_result_digest.as_deref(), Some("digest-1"));
        assert_eq!(merged.python_result_digest.as_deref(), Some("digest-1"));
    }

    #[test]
    fn marks_mismatch_when_rust_and_python_digests_differ() {
        let existing = stored(merge_shadow_result_sample(
            None,
            rust_sample("digest-1", 100),
        ));
        let merged = merge_shadow_result_sample(Some(&existing), python_sample("digest-2", 200));

        assert_eq!(merged.match_status, ShadowResultMatchStatus::Mismatch);
        assert_eq!(
            merged.error_message.as_deref(),
            Some("shadow result digest mismatch")
        );
        assert_eq!(merged.request_id.as_deref(), Some("req-1"));
        assert_eq!(merged.rust_result_digest.as_deref(), Some("digest-1"));
        assert_eq!(merged.python_result_digest.as_deref(), Some("digest-2"));
    }
}
