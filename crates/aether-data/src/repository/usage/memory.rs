use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{StoredRequestUsageAudit, UsageReadRepository};
use crate::DataLayerError;

#[derive(Debug, Default)]
pub struct InMemoryUsageReadRepository {
    by_request_id: RwLock<BTreeMap<String, StoredRequestUsageAudit>>,
}

impl InMemoryUsageReadRepository {
    pub fn seed<I>(items: I) -> Self
    where
        I: IntoIterator<Item = StoredRequestUsageAudit>,
    {
        let mut by_request_id = BTreeMap::new();
        for item in items {
            by_request_id.insert(item.request_id.clone(), item);
        }
        Self {
            by_request_id: RwLock::new(by_request_id),
        }
    }
}

#[async_trait]
impl UsageReadRepository for InMemoryUsageReadRepository {
    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<StoredRequestUsageAudit>, DataLayerError> {
        Ok(self
            .by_request_id
            .read()
            .expect("usage repository lock")
            .get(request_id)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryUsageReadRepository;
    use crate::repository::usage::{StoredRequestUsageAudit, UsageReadRepository};

    fn sample_usage(request_id: &str, created_at_unix_secs: i64) -> StoredRequestUsageAudit {
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
            100,
            50,
            150,
            0.12,
            0.18,
            Some(200),
            None,
            None,
            Some(420),
            Some(120),
            "completed".to_string(),
            "settled".to_string(),
            created_at_unix_secs,
            created_at_unix_secs + 1,
            Some(created_at_unix_secs + 2),
        )
        .expect("usage should build")
    }

    #[tokio::test]
    async fn finds_usage_by_request_id() {
        let repository = InMemoryUsageReadRepository::seed(vec![
            sample_usage("req-1", 100),
            sample_usage("req-2", 200),
        ]);

        let usage = repository
            .find_by_request_id("req-2")
            .await
            .expect("find should succeed")
            .expect("usage should exist");

        assert_eq!(usage.request_id, "req-2");
        assert_eq!(usage.total_tokens, 150);
    }
}
