use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{RequestCandidateReadRepository, StoredRequestCandidate};
use crate::DataLayerError;

#[derive(Debug, Default)]
pub struct InMemoryRequestCandidateRepository {
    by_id: RwLock<BTreeMap<String, StoredRequestCandidate>>,
}

impl InMemoryRequestCandidateRepository {
    pub fn seed<I>(items: I) -> Self
    where
        I: IntoIterator<Item = StoredRequestCandidate>,
    {
        let mut by_id = BTreeMap::new();
        for item in items {
            by_id.insert(item.id.clone(), item);
        }
        Self {
            by_id: RwLock::new(by_id),
        }
    }
}

#[async_trait]
impl RequestCandidateReadRepository for InMemoryRequestCandidateRepository {
    async fn list_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        let mut rows = self
            .by_id
            .read()
            .expect("request candidate repository lock")
            .values()
            .filter(|row| row.request_id == request_id)
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.candidate_index
                .cmp(&right.candidate_index)
                .then(left.retry_index.cmp(&right.retry_index))
                .then(left.created_at_unix_secs.cmp(&right.created_at_unix_secs))
        });
        Ok(rows)
    }

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut rows = self
            .by_id
            .read()
            .expect("request candidate repository lock")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| right.created_at_unix_secs.cmp(&left.created_at_unix_secs));
        rows.truncate(limit);
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryRequestCandidateRepository;
    use crate::repository::candidates::{
        RequestCandidateReadRepository, RequestCandidateStatus, StoredRequestCandidate,
    };

    fn sample_candidate(
        id: &str,
        request_id: &str,
        created_at_unix_secs: i64,
    ) -> StoredRequestCandidate {
        StoredRequestCandidate::new(
            id.to_string(),
            request_id.to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            Some("alice".to_string()),
            Some("default".to_string()),
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Success,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            Some(1),
            None,
            None,
            created_at_unix_secs,
            Some(created_at_unix_secs),
            Some(created_at_unix_secs + 1),
        )
        .expect("candidate should build")
    }

    #[tokio::test]
    async fn lists_request_candidates_by_request_id_in_candidate_order() {
        let repository = InMemoryRequestCandidateRepository::seed(vec![
            sample_candidate("cand-2", "req-1", 200),
            sample_candidate("cand-1", "req-1", 100),
            sample_candidate("cand-3", "req-2", 300),
        ]);

        let rows = repository
            .list_by_request_id("req-1")
            .await
            .expect("list should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].request_id, "req-1");
        assert_eq!(rows[1].request_id, "req-1");
    }

    #[tokio::test]
    async fn lists_recent_request_candidates_in_descending_created_order() {
        let repository = InMemoryRequestCandidateRepository::seed(vec![
            sample_candidate("cand-1", "req-1", 100),
            sample_candidate("cand-2", "req-2", 200),
        ]);

        let rows = repository
            .list_recent(10)
            .await
            .expect("list recent should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "cand-2");
        assert_eq!(rows[1].id, "cand-1");
    }
}
