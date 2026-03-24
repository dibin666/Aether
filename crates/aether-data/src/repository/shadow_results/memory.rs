use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{
    ShadowResultLookupKey, ShadowResultReadRepository, ShadowResultWriteRepository,
    StoredShadowResult, UpsertShadowResult,
};
use crate::DataLayerError;

#[derive(Debug, Default)]
pub struct InMemoryShadowResultRepository {
    results: RwLock<BTreeMap<(String, String), StoredShadowResult>>,
}

#[async_trait]
impl ShadowResultReadRepository for InMemoryShadowResultRepository {
    async fn find(
        &self,
        key: ShadowResultLookupKey<'_>,
    ) -> Result<Option<StoredShadowResult>, DataLayerError> {
        let results = self.results.read().expect("shadow result repository lock");
        Ok(match key {
            ShadowResultLookupKey::TraceFingerprint {
                trace_id,
                request_fingerprint,
            } => results
                .get(&(trace_id.to_string(), request_fingerprint.to_string()))
                .cloned(),
        })
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<StoredShadowResult>, DataLayerError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut results = self
            .results
            .read()
            .expect("shadow result repository lock")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        results.sort_by(|left, right| right.updated_at_unix_secs.cmp(&left.updated_at_unix_secs));
        results.truncate(limit);
        Ok(results)
    }
}

#[async_trait]
impl ShadowResultWriteRepository for InMemoryShadowResultRepository {
    async fn upsert(
        &self,
        result: UpsertShadowResult,
    ) -> Result<StoredShadowResult, DataLayerError> {
        let stored = result.into_stored();
        let mut results = self.results.write().expect("shadow result repository lock");
        results.insert(
            (stored.trace_id.clone(), stored.request_fingerprint.clone()),
            stored.clone(),
        );
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryShadowResultRepository;
    use crate::repository::shadow_results::{
        ShadowResultLookupKey, ShadowResultMatchStatus, ShadowResultReadRepository,
        ShadowResultWriteRepository, UpsertShadowResult,
    };

    fn sample_result(
        trace_id: &str,
        request_fingerprint: &str,
        updated_at_unix_secs: u64,
    ) -> UpsertShadowResult {
        UpsertShadowResult {
            trace_id: trace_id.to_string(),
            request_fingerprint: request_fingerprint.to_string(),
            request_id: Some(format!("req-{trace_id}")),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: Some("cand-1".to_string()),
            rust_result_digest: Some("rust-digest".to_string()),
            python_result_digest: Some("python-digest".to_string()),
            match_status: ShadowResultMatchStatus::Match,
            status_code: Some(200),
            error_message: None,
            created_at_unix_secs: updated_at_unix_secs.saturating_sub(10),
            updated_at_unix_secs,
        }
    }

    #[tokio::test]
    async fn reads_result_by_trace_and_fingerprint() {
        let repo = InMemoryShadowResultRepository::default();
        repo.upsert(sample_result("trace-1", "fp-1", 100))
            .await
            .expect("upsert should succeed");

        assert!(repo
            .find(ShadowResultLookupKey::TraceFingerprint {
                trace_id: "trace-1",
                request_fingerprint: "fp-1",
            })
            .await
            .expect("find should succeed")
            .is_some());
    }

    #[tokio::test]
    async fn list_recent_returns_results_in_descending_update_order() {
        let repo = InMemoryShadowResultRepository::default();
        repo.upsert(sample_result("trace-1", "fp-1", 100))
            .await
            .expect("upsert should succeed");
        repo.upsert(sample_result("trace-2", "fp-2", 200))
            .await
            .expect("upsert should succeed");

        let recent = repo
            .list_recent(10)
            .await
            .expect("list recent should succeed");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].trace_id, "trace-2");
        assert_eq!(recent[1].trace_id, "trace-1");
    }

    #[tokio::test]
    async fn upsert_replaces_existing_shadow_result() {
        let repo = InMemoryShadowResultRepository::default();
        repo.upsert(sample_result("trace-1", "fp-1", 100))
            .await
            .expect("upsert should succeed");
        repo.upsert(UpsertShadowResult {
            trace_id: "trace-1".to_string(),
            request_fingerprint: "fp-1".to_string(),
            request_id: Some("req-trace-1".to_string()),
            route_family: Some("openai".to_string()),
            route_kind: Some("chat".to_string()),
            candidate_id: Some("cand-2".to_string()),
            rust_result_digest: Some("rust-digest-2".to_string()),
            python_result_digest: Some("python-digest-2".to_string()),
            match_status: ShadowResultMatchStatus::Mismatch,
            status_code: Some(502),
            error_message: Some("mismatch".to_string()),
            created_at_unix_secs: 100,
            updated_at_unix_secs: 200,
        })
        .await
        .expect("upsert should succeed");

        let stored = repo
            .find(ShadowResultLookupKey::TraceFingerprint {
                trace_id: "trace-1",
                request_fingerprint: "fp-1",
            })
            .await
            .expect("find should succeed")
            .expect("stored result should exist");
        assert_eq!(stored.request_id.as_deref(), Some("req-trace-1"));
        assert_eq!(stored.candidate_id.as_deref(), Some("cand-2"));
        assert_eq!(stored.match_status, ShadowResultMatchStatus::Mismatch);
    }
}
