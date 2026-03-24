use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{
    StoredVideoTask, UpsertVideoTask, VideoTaskLookupKey, VideoTaskReadRepository,
    VideoTaskWriteRepository,
};
use crate::DataLayerError;

#[derive(Debug, Default)]
struct MemoryVideoTaskIndex {
    by_id: BTreeMap<String, StoredVideoTask>,
    short_to_id: BTreeMap<String, String>,
    user_external_to_id: BTreeMap<(String, String), String>,
}

#[derive(Debug, Default)]
pub struct InMemoryVideoTaskRepository {
    index: RwLock<MemoryVideoTaskIndex>,
}

impl InMemoryVideoTaskRepository {
    fn store_locked(index: &mut MemoryVideoTaskIndex, task: StoredVideoTask) -> StoredVideoTask {
        if let Some(previous) = index.by_id.insert(task.id.clone(), task.clone()) {
            if let Some(short_id) = previous.short_id {
                index.short_to_id.remove(&short_id);
            }
            if let (Some(user_id), Some(external_task_id)) =
                (previous.user_id, previous.external_task_id)
            {
                index
                    .user_external_to_id
                    .remove(&(user_id, external_task_id));
            }
        }

        if let Some(short_id) = &task.short_id {
            index.short_to_id.insert(short_id.clone(), task.id.clone());
        }
        if let (Some(user_id), Some(external_task_id)) = (&task.user_id, &task.external_task_id) {
            index
                .user_external_to_id
                .insert((user_id.clone(), external_task_id.clone()), task.id.clone());
        }

        task
    }
}

#[async_trait]
impl VideoTaskReadRepository for InMemoryVideoTaskRepository {
    async fn find(
        &self,
        key: VideoTaskLookupKey<'_>,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        let index = self.index.read().expect("video task repository lock");
        Ok(match key {
            VideoTaskLookupKey::Id(id) => index.by_id.get(id).cloned(),
            VideoTaskLookupKey::ShortId(short_id) => index
                .short_to_id
                .get(short_id)
                .and_then(|id| index.by_id.get(id))
                .cloned(),
            VideoTaskLookupKey::UserExternal {
                user_id,
                external_task_id,
            } => index
                .user_external_to_id
                .get(&(user_id.to_string(), external_task_id.to_string()))
                .and_then(|id| index.by_id.get(id))
                .cloned(),
        })
    }

    async fn list_active(&self, limit: usize) -> Result<Vec<StoredVideoTask>, DataLayerError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut tasks = self
            .index
            .read()
            .expect("video task repository lock")
            .by_id
            .values()
            .filter(|task| task.status.is_active())
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at_unix_secs.cmp(&left.updated_at_unix_secs));
        tasks.truncate(limit);
        Ok(tasks)
    }
}

#[async_trait]
impl VideoTaskWriteRepository for InMemoryVideoTaskRepository {
    async fn upsert(&self, task: UpsertVideoTask) -> Result<StoredVideoTask, DataLayerError> {
        let mut index = self.index.write().expect("video task repository lock");
        Ok(Self::store_locked(&mut index, task.into_stored()))
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryVideoTaskRepository;
    use crate::repository::video_tasks::{
        UpsertVideoTask, VideoTaskLookupKey, VideoTaskReadRepository, VideoTaskStatus,
        VideoTaskWriteRepository,
    };

    fn sample_task(
        id: &str,
        status: VideoTaskStatus,
        updated_at_unix_secs: u64,
    ) -> UpsertVideoTask {
        UpsertVideoTask {
            id: id.to_string(),
            short_id: Some(format!("short-{id}")),
            user_id: Some("user-1".to_string()),
            external_task_id: Some(format!("ext-{id}")),
            provider_api_format: Some("openai:video".to_string()),
            model: Some("sora-2".to_string()),
            prompt: Some("hello".to_string()),
            size: Some("1280x720".to_string()),
            status,
            progress_percent: 0,
            created_at_unix_secs: updated_at_unix_secs.saturating_sub(10),
            updated_at_unix_secs,
            error_code: None,
            error_message: None,
            video_url: None,
        }
    }

    #[tokio::test]
    async fn reads_task_by_all_supported_lookup_keys() {
        let repo = InMemoryVideoTaskRepository::default();
        repo.upsert(sample_task("task-1", VideoTaskStatus::Submitted, 100))
            .await
            .expect("upsert should succeed");

        assert!(repo
            .find(VideoTaskLookupKey::Id("task-1"))
            .await
            .expect("find by id should succeed")
            .is_some());
        assert!(repo
            .find(VideoTaskLookupKey::ShortId("short-task-1"))
            .await
            .expect("find by short id should succeed")
            .is_some());
        assert!(repo
            .find(VideoTaskLookupKey::UserExternal {
                user_id: "user-1",
                external_task_id: "ext-task-1",
            })
            .await
            .expect("find by user/external should succeed")
            .is_some());
    }

    #[tokio::test]
    async fn list_active_only_returns_active_tasks_in_descending_update_order() {
        let repo = InMemoryVideoTaskRepository::default();
        repo.upsert(sample_task("task-1", VideoTaskStatus::Completed, 100))
            .await
            .expect("upsert should succeed");
        repo.upsert(sample_task("task-2", VideoTaskStatus::Processing, 200))
            .await
            .expect("upsert should succeed");
        repo.upsert(sample_task("task-3", VideoTaskStatus::Queued, 150))
            .await
            .expect("upsert should succeed");

        let active = repo
            .list_active(10)
            .await
            .expect("list active should succeed");
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].id, "task-2");
        assert_eq!(active[1].id, "task-3");
    }

    #[tokio::test]
    async fn upsert_replaces_secondary_indexes() {
        let repo = InMemoryVideoTaskRepository::default();
        repo.upsert(sample_task("task-1", VideoTaskStatus::Submitted, 100))
            .await
            .expect("upsert should succeed");

        repo.upsert(UpsertVideoTask {
            id: "task-1".to_string(),
            short_id: Some("short-task-1b".to_string()),
            user_id: Some("user-2".to_string()),
            external_task_id: Some("ext-task-1b".to_string()),
            provider_api_format: Some("gemini:video".to_string()),
            model: Some("veo-3".to_string()),
            prompt: Some("remix".to_string()),
            size: Some("720p".to_string()),
            status: VideoTaskStatus::Processing,
            progress_percent: 50,
            created_at_unix_secs: 150,
            updated_at_unix_secs: 200,
            error_code: None,
            error_message: None,
            video_url: None,
        })
        .await
        .expect("upsert should succeed");

        assert!(repo
            .find(VideoTaskLookupKey::ShortId("short-task-1"))
            .await
            .expect("find should succeed")
            .is_none());
        assert!(repo
            .find(VideoTaskLookupKey::UserExternal {
                user_id: "user-1",
                external_task_id: "ext-task-1",
            })
            .await
            .expect("find should succeed")
            .is_none());
        assert!(repo
            .find(VideoTaskLookupKey::ShortId("short-task-1b"))
            .await
            .expect("find should succeed")
            .is_some());
    }
}
