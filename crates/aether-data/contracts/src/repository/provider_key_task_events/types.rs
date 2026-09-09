use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderKeyTaskEvent {
    pub id: String,
    pub task_key: String,
    pub task_run_id: String,
    pub event_type: String,
    pub provider_id: String,
    pub provider_name: Option<String>,
    pub provider_type: Option<String>,
    pub provider_api_key_id: String,
    pub provider_api_key_name: Option<String>,
    pub action: String,
    pub status: String,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub created_at_unix_secs: u64,
}

impl ProviderKeyTaskEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_key: impl Into<String>,
        task_run_id: impl Into<String>,
        event_type: impl Into<String>,
        provider_id: impl Into<String>,
        provider_api_key_id: impl Into<String>,
        action: impl Into<String>,
        status: impl Into<String>,
        created_at_unix_secs: u64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_key: task_key.into(),
            task_run_id: task_run_id.into(),
            event_type: event_type.into(),
            provider_id: provider_id.into(),
            provider_name: None,
            provider_type: None,
            provider_api_key_id: provider_api_key_id.into(),
            provider_api_key_name: None,
            action: action.into(),
            status: status.into(),
            message: None,
            reason: None,
            created_at_unix_secs,
        }
    }

    pub fn with_provider_name(mut self, name: Option<impl Into<String>>) -> Self {
        self.provider_name = name.map(Into::into);
        self
    }

    pub fn with_provider_type(mut self, provider_type: Option<impl Into<String>>) -> Self {
        self.provider_type = provider_type.map(Into::into);
        self
    }

    pub fn with_provider_api_key_name(mut self, name: Option<impl Into<String>>) -> Self {
        self.provider_api_key_name = name.map(Into::into);
        self
    }

    pub fn with_message(mut self, message: Option<impl Into<String>>) -> Self {
        self.message = message.map(Into::into);
        self
    }

    pub fn with_reason(mut self, reason: Option<impl Into<String>>) -> Self {
        self.reason = reason.map(Into::into);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderKeyTaskEventQuery {
    pub task_key: String,
    pub task_run_id: Option<String>,
    pub limit: Option<usize>,
    pub descending: bool,
}

impl ProviderKeyTaskEventQuery {
    pub fn new(task_key: impl Into<String>) -> Self {
        Self {
            task_key: task_key.into(),
            task_run_id: None,
            limit: None,
            descending: true,
        }
    }

    pub fn with_run_id(mut self, task_run_id: impl Into<String>) -> Self {
        self.task_run_id = Some(task_run_id.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_descending(mut self, descending: bool) -> Self {
        self.descending = descending;
        self
    }
}

#[async_trait]
pub trait ProviderKeyTaskEventReadRepository: Send + Sync {
    async fn list_provider_key_task_events(
        &self,
        query: &ProviderKeyTaskEventQuery,
    ) -> Result<Vec<ProviderKeyTaskEvent>, crate::DataLayerError>;
}

#[async_trait]
pub trait ProviderKeyTaskEventWriteRepository: Send + Sync {
    async fn append_provider_key_task_events(
        &self,
        events: &[ProviderKeyTaskEvent],
    ) -> Result<usize, crate::DataLayerError>;

    async fn delete_provider_key_task_events_before(
        &self,
        cutoff_unix_secs: u64,
    ) -> Result<usize, crate::DataLayerError>;
}

pub trait ProviderKeyTaskEventRepository:
    ProviderKeyTaskEventReadRepository + ProviderKeyTaskEventWriteRepository + Send + Sync
{
}

impl<T> ProviderKeyTaskEventRepository for T where
    T: ProviderKeyTaskEventReadRepository + ProviderKeyTaskEventWriteRepository + Send + Sync
{
}
