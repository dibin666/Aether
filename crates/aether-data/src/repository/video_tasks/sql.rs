use sqlx::{PgPool, Row};

use async_trait::async_trait;

use crate::repository::video_tasks::{
    StoredVideoTask, VideoTaskLookupKey, VideoTaskReadRepository, VideoTaskStatus,
};
use crate::DataLayerError;

const FIND_BY_ID_SQL: &str = r#"
SELECT
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
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM updated_at) AS BIGINT) AS updated_at_unix_secs
  ,
  error_code,
  error_message,
  video_url
FROM video_tasks
WHERE id = $1
LIMIT 1
"#;

const FIND_BY_SHORT_ID_SQL: &str = r#"
SELECT
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
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM updated_at) AS BIGINT) AS updated_at_unix_secs
  ,
  error_code,
  error_message,
  video_url
FROM video_tasks
WHERE short_id = $1
LIMIT 1
"#;

const FIND_BY_USER_EXTERNAL_SQL: &str = r#"
SELECT
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
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM updated_at) AS BIGINT) AS updated_at_unix_secs
  ,
  error_code,
  error_message,
  video_url
FROM video_tasks
WHERE user_id = $1 AND external_task_id = $2
LIMIT 1
"#;

const LIST_ACTIVE_SQL: &str = r#"
SELECT
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
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM updated_at) AS BIGINT) AS updated_at_unix_secs
  ,
  error_code,
  error_message,
  video_url
FROM video_tasks
WHERE status = ANY($1)
ORDER BY updated_at DESC
LIMIT $2
"#;

#[derive(Debug, Clone)]
pub struct SqlxVideoTaskReadRepository {
    pool: PgPool,
}

impl SqlxVideoTaskReadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn find(
        &self,
        key: VideoTaskLookupKey<'_>,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        match key {
            VideoTaskLookupKey::Id(id) => self.find_by_id(id).await,
            VideoTaskLookupKey::ShortId(short_id) => self.find_by_short_id(short_id).await,
            VideoTaskLookupKey::UserExternal {
                user_id,
                external_task_id,
            } => self.find_by_user_external(user_id, external_task_id).await,
        }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<StoredVideoTask>, DataLayerError> {
        let row = sqlx::query(FIND_BY_ID_SQL)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_video_task_row).transpose()
    }

    pub async fn find_by_short_id(
        &self,
        short_id: &str,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        let row = sqlx::query(FIND_BY_SHORT_ID_SQL)
            .bind(short_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_video_task_row).transpose()
    }

    pub async fn find_by_user_external(
        &self,
        user_id: &str,
        external_task_id: &str,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        let row = sqlx::query(FIND_BY_USER_EXTERNAL_SQL)
            .bind(user_id)
            .bind(external_task_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_video_task_row).transpose()
    }

    pub async fn list_active(&self, limit: usize) -> Result<Vec<StoredVideoTask>, DataLayerError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let active_statuses = vec!["pending", "submitted", "queued", "processing"];
        let rows = sqlx::query(LIST_ACTIVE_SQL)
            .bind(active_statuses)
            .bind(i64::try_from(limit).map_err(|_| {
                DataLayerError::UnexpectedValue(format!("invalid active task limit: {limit}"))
            })?)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(map_video_task_row).collect()
    }
}

#[async_trait]
impl VideoTaskReadRepository for SqlxVideoTaskReadRepository {
    async fn find(
        &self,
        key: VideoTaskLookupKey<'_>,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        Self::find(self, key).await
    }

    async fn list_active(&self, limit: usize) -> Result<Vec<StoredVideoTask>, DataLayerError> {
        Self::list_active(self, limit).await
    }
}

fn map_video_task_row(row: &sqlx::postgres::PgRow) -> Result<StoredVideoTask, DataLayerError> {
    let status = VideoTaskStatus::from_database(row.try_get::<String, _>("status")?.as_str())?;
    StoredVideoTask::new(
        row.try_get("id")?,
        row.try_get("short_id")?,
        row.try_get("user_id")?,
        row.try_get("external_task_id")?,
        row.try_get("provider_api_format")?,
        row.try_get("model")?,
        row.try_get("prompt")?,
        row.try_get("size")?,
        status,
        row.try_get("progress_percent")?,
        row.try_get("created_at_unix_secs")?,
        row.try_get("updated_at_unix_secs")?,
        row.try_get("error_code")?,
        row.try_get("error_message")?,
        row.try_get("video_url")?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxVideoTaskReadRepository;
    use crate::postgres::{PostgresPoolConfig, PostgresPoolFactory};
    use crate::repository::video_tasks::{VideoTaskLookupKey, VideoTaskReadRepository};

    #[tokio::test]
    async fn repository_constructs_from_lazy_pool() {
        let factory = PostgresPoolFactory::new(PostgresPoolConfig {
            database_url: "postgres://localhost/aether".to_string(),
            min_connections: 1,
            max_connections: 4,
            acquire_timeout_ms: 1_000,
            idle_timeout_ms: 5_000,
            max_lifetime_ms: 30_000,
            statement_cache_capacity: 64,
            require_ssl: false,
        })
        .expect("factory should build");

        let pool = factory.connect_lazy().expect("pool should build");
        let repository = SqlxVideoTaskReadRepository::new(pool);
        let _ = repository.pool();
    }

    #[tokio::test]
    async fn read_trait_delegates_to_sqlx_repository() {
        let factory = PostgresPoolFactory::new(PostgresPoolConfig {
            database_url: "postgres://localhost/aether".to_string(),
            min_connections: 1,
            max_connections: 4,
            acquire_timeout_ms: 1_000,
            idle_timeout_ms: 5_000,
            max_lifetime_ms: 30_000,
            statement_cache_capacity: 64,
            require_ssl: false,
        })
        .expect("factory should build");

        let pool = factory.connect_lazy().expect("pool should build");
        let repository = SqlxVideoTaskReadRepository::new(pool);
        let _ = VideoTaskReadRepository::find(&repository, VideoTaskLookupKey::Id("task-1")).await;
    }
}
