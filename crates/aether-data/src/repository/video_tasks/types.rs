use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VideoTaskStatus {
    Pending,
    Submitted,
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
    Expired,
    Deleted,
}

impl VideoTaskStatus {
    pub fn from_database(value: &str) -> Result<Self, crate::DataLayerError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "submitted" => Ok(Self::Submitted),
            "queued" => Ok(Self::Queued),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "expired" => Ok(Self::Expired),
            "deleted" => Ok(Self::Deleted),
            other => Err(crate::DataLayerError::UnexpectedValue(format!(
                "unsupported video_tasks.status: {other}"
            ))),
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Submitted | Self::Queued | Self::Processing
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredVideoTask {
    pub id: String,
    pub short_id: Option<String>,
    pub user_id: Option<String>,
    pub external_task_id: Option<String>,
    pub provider_api_format: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub size: Option<String>,
    pub status: VideoTaskStatus,
    pub progress_percent: u16,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub video_url: Option<String>,
}

impl StoredVideoTask {
    pub fn new(
        id: String,
        short_id: Option<String>,
        user_id: Option<String>,
        external_task_id: Option<String>,
        provider_api_format: Option<String>,
        model: Option<String>,
        prompt: Option<String>,
        size: Option<String>,
        status: VideoTaskStatus,
        progress_percent: i32,
        created_at_unix_secs: i64,
        updated_at_unix_secs: i64,
        error_code: Option<String>,
        error_message: Option<String>,
        video_url: Option<String>,
    ) -> Result<Self, crate::DataLayerError> {
        let progress_percent = u16::try_from(progress_percent).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid progress_percent: {progress_percent}"
            ))
        })?;
        let created_at_unix_secs = u64::try_from(created_at_unix_secs).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid created_at_unix_secs: {created_at_unix_secs}"
            ))
        })?;
        let updated_at_unix_secs = u64::try_from(updated_at_unix_secs).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid updated_at_unix_secs: {updated_at_unix_secs}"
            ))
        })?;

        Ok(Self {
            id,
            short_id,
            user_id,
            external_task_id,
            provider_api_format,
            model,
            prompt,
            size,
            status,
            progress_percent,
            created_at_unix_secs,
            updated_at_unix_secs,
            error_code,
            error_message,
            video_url,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertVideoTask {
    pub id: String,
    pub short_id: Option<String>,
    pub user_id: Option<String>,
    pub external_task_id: Option<String>,
    pub provider_api_format: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub size: Option<String>,
    pub status: VideoTaskStatus,
    pub progress_percent: u16,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub video_url: Option<String>,
}

impl UpsertVideoTask {
    pub fn into_stored(self) -> StoredVideoTask {
        StoredVideoTask {
            id: self.id,
            short_id: self.short_id,
            user_id: self.user_id,
            external_task_id: self.external_task_id,
            provider_api_format: self.provider_api_format,
            model: self.model,
            prompt: self.prompt,
            size: self.size,
            status: self.status,
            progress_percent: self.progress_percent,
            created_at_unix_secs: self.created_at_unix_secs,
            updated_at_unix_secs: self.updated_at_unix_secs,
            error_code: self.error_code,
            error_message: self.error_message,
            video_url: self.video_url,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoTaskLookupKey<'a> {
    Id(&'a str),
    ShortId(&'a str),
    UserExternal {
        user_id: &'a str,
        external_task_id: &'a str,
    },
}

#[async_trait]
pub trait VideoTaskReadRepository: Send + Sync {
    async fn find(
        &self,
        key: VideoTaskLookupKey<'_>,
    ) -> Result<Option<StoredVideoTask>, crate::DataLayerError>;

    async fn list_active(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredVideoTask>, crate::DataLayerError>;
}

#[async_trait]
pub trait VideoTaskWriteRepository: Send + Sync {
    async fn upsert(&self, task: UpsertVideoTask)
        -> Result<StoredVideoTask, crate::DataLayerError>;
}

pub trait VideoTaskRepository:
    VideoTaskReadRepository + VideoTaskWriteRepository + Send + Sync
{
}

impl<T> VideoTaskRepository for T where
    T: VideoTaskReadRepository + VideoTaskWriteRepository + Send + Sync
{
}

#[cfg(test)]
mod tests {
    use super::{StoredVideoTask, VideoTaskStatus};

    #[test]
    fn parses_status_from_database_text() {
        assert_eq!(
            VideoTaskStatus::from_database("processing").expect("status should parse"),
            VideoTaskStatus::Processing
        );
    }

    #[test]
    fn rejects_invalid_database_status() {
        assert!(VideoTaskStatus::from_database("mystery").is_err());
    }

    #[test]
    fn rejects_invalid_numeric_fields() {
        assert!(StoredVideoTask::new(
            "task-1".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            VideoTaskStatus::Submitted,
            -1,
            1,
            1,
            None,
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn rejects_negative_updated_at_values() {
        assert!(StoredVideoTask::new(
            "task-1".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            VideoTaskStatus::Submitted,
            10,
            1,
            -1,
            None,
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn rejects_negative_created_at_values() {
        assert!(StoredVideoTask::new(
            "task-1".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            VideoTaskStatus::Submitted,
            10,
            -1,
            1,
            None,
            None,
            None
        )
        .is_err());
    }
}
