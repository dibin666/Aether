use std::sync::RwLock;

use async_trait::async_trait;

use super::{
    ProviderKeyTaskEvent, ProviderKeyTaskEventQuery, ProviderKeyTaskEventReadRepository,
    ProviderKeyTaskEventWriteRepository,
};
use crate::DataLayerError;

#[derive(Debug, Default)]
pub struct InMemoryProviderKeyTaskEventRepository {
    events: RwLock<Vec<ProviderKeyTaskEvent>>,
}

impl InMemoryProviderKeyTaskEventRepository {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
        }
    }

    pub fn seed<I>(items: I) -> Self
    where
        I: IntoIterator<Item = ProviderKeyTaskEvent>,
    {
        Self {
            events: RwLock::new(items.into_iter().collect()),
        }
    }
}

#[async_trait]
impl ProviderKeyTaskEventReadRepository for InMemoryProviderKeyTaskEventRepository {
    async fn list_provider_key_task_events(
        &self,
        query: &ProviderKeyTaskEventQuery,
    ) -> Result<Vec<ProviderKeyTaskEvent>, DataLayerError> {
        let events = self.events.read().expect("provider key task events lock");
        let mut filtered: Vec<ProviderKeyTaskEvent> = events
            .iter()
            .filter(|e| e.task_key == query.task_key)
            .filter(|e| {
                query
                    .task_run_id
                    .as_ref()
                    .is_none_or(|run_id| &e.task_run_id == run_id)
            })
            .cloned()
            .collect();
        filtered.sort_by(|a, b| {
            if query.descending {
                b.created_at_unix_secs
                    .cmp(&a.created_at_unix_secs)
                    .then_with(|| b.id.cmp(&a.id))
            } else {
                a.created_at_unix_secs
                    .cmp(&b.created_at_unix_secs)
                    .then_with(|| a.id.cmp(&b.id))
            }
        });
        if let Some(limit) = query.limit {
            filtered.truncate(limit);
        }
        Ok(filtered)
    }
}

#[async_trait]
impl ProviderKeyTaskEventWriteRepository for InMemoryProviderKeyTaskEventRepository {
    async fn append_provider_key_task_events(
        &self,
        events: &[ProviderKeyTaskEvent],
    ) -> Result<usize, DataLayerError> {
        if events.is_empty() {
            return Ok(0);
        }
        let mut store = self.events.write().expect("provider key task events lock");
        let mut appended = 0usize;
        for event in events {
            if store.iter().any(|e| e.id == event.id) {
                continue;
            }
            store.push(event.clone());
            appended += 1;
        }
        Ok(appended)
    }

    async fn delete_provider_key_task_events_before(
        &self,
        cutoff_unix_secs: u64,
    ) -> Result<usize, DataLayerError> {
        let mut store = self.events.write().expect("provider key task events lock");
        let initial_len = store.len();
        store.retain(|e| e.created_at_unix_secs >= cutoff_unix_secs);
        Ok(initial_len - store.len())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryProviderKeyTaskEventRepository;
    use crate::repository::provider_key_task_events::{
        ProviderKeyTaskEvent, ProviderKeyTaskEventQuery, ProviderKeyTaskEventReadRepository,
        ProviderKeyTaskEventWriteRepository,
    };

    fn sample_event(
        id: &str,
        task_key: &str,
        run_id: &str,
        created_at: u64,
    ) -> ProviderKeyTaskEvent {
        ProviderKeyTaskEvent {
            id: id.to_string(),
            task_key: task_key.to_string(),
            task_run_id: run_id.to_string(),
            event_type: "oauth_refresh_account_refreshed".to_string(),
            provider_id: "provider-1".to_string(),
            provider_name: Some("Provider One".to_string()),
            provider_type: Some("codex".to_string()),
            provider_api_key_id: "key-1".to_string(),
            provider_api_key_name: Some("Key One".to_string()),
            action: "oauth_refresh".to_string(),
            status: "refreshed".to_string(),
            message: Some("Token refreshed".to_string()),
            reason: None,
            created_at_unix_secs: created_at,
        }
    }

    #[tokio::test]
    async fn appends_and_lists_events_with_filters() {
        let repo = InMemoryProviderKeyTaskEventRepository::new();
        let events = vec![
            sample_event("e1", "task_a", "run_1", 100),
            sample_event("e2", "task_a", "run_1", 200),
            sample_event("e3", "task_a", "run_2", 300),
            sample_event("e4", "task_b", "run_3", 400),
        ];
        let appended = repo
            .append_provider_key_task_events(&events)
            .await
            .expect("append should succeed");
        assert_eq!(appended, 4);

        // Query by task_key alone, default descending
        let res = repo
            .list_provider_key_task_events(&ProviderKeyTaskEventQuery::new("task_a"))
            .await
            .expect("query should succeed");
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].id, "e3");
        assert_eq!(res[1].id, "e2");
        assert_eq!(res[2].id, "e1");

        // Query with run_id filter
        let res_run1 = repo
            .list_provider_key_task_events(
                &ProviderKeyTaskEventQuery::new("task_a").with_run_id("run_1"),
            )
            .await
            .expect("query should succeed");
        assert_eq!(res_run1.len(), 2);
        assert_eq!(res_run1[0].id, "e2");
        assert_eq!(res_run1[1].id, "e1");

        // Query with limit
        let res_limit = repo
            .list_provider_key_task_events(&ProviderKeyTaskEventQuery::new("task_a").with_limit(1))
            .await
            .expect("query should succeed");
        assert_eq!(res_limit.len(), 1);
        assert_eq!(res_limit[0].id, "e3");

        // Query ascending
        let res_asc = repo
            .list_provider_key_task_events(
                &ProviderKeyTaskEventQuery::new("task_a").with_descending(false),
            )
            .await
            .expect("query should succeed");
        assert_eq!(res_asc[0].id, "e1");
        assert_eq!(res_asc[2].id, "e3");
    }

    #[tokio::test]
    async fn deletes_events_before_cutoff() {
        let repo = InMemoryProviderKeyTaskEventRepository::seed(vec![
            sample_event("e1", "task_a", "run_1", 100),
            sample_event("e2", "task_a", "run_1", 200),
            sample_event("e3", "task_a", "run_2", 300),
        ]);

        let deleted = repo
            .delete_provider_key_task_events_before(200)
            .await
            .expect("delete should succeed");
        assert_eq!(deleted, 1); // e1 is < 200

        let remaining = repo
            .list_provider_key_task_events(&ProviderKeyTaskEventQuery::new("task_a"))
            .await
            .expect("query should succeed");
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].id, "e3");
        assert_eq!(remaining[1].id, "e2");
    }
}
