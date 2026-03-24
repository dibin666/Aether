use async_trait::async_trait;
use sqlx::{PgPool, Row};

use super::types::{
    RequestCandidateReadRepository, RequestCandidateStatus, StoredRequestCandidate,
};
use crate::DataLayerError;

const LIST_BY_REQUEST_ID_SQL: &str = r#"
SELECT
  id,
  request_id,
  user_id,
  api_key_id,
  username,
  api_key_name,
  candidate_index,
  retry_index,
  provider_id,
  endpoint_id,
  key_id,
  status,
  skip_reason,
  is_cached,
  status_code,
  error_type,
  error_message,
  latency_ms,
  concurrent_requests,
  extra_data,
  required_capabilities,
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM started_at) AS BIGINT) AS started_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM finished_at) AS BIGINT) AS finished_at_unix_secs
FROM request_candidates
WHERE request_id = $1
ORDER BY candidate_index ASC, retry_index ASC, created_at ASC
"#;

const LIST_RECENT_SQL: &str = r#"
SELECT
  id,
  request_id,
  user_id,
  api_key_id,
  username,
  api_key_name,
  candidate_index,
  retry_index,
  provider_id,
  endpoint_id,
  key_id,
  status,
  skip_reason,
  is_cached,
  status_code,
  error_type,
  error_message,
  latency_ms,
  concurrent_requests,
  extra_data,
  required_capabilities,
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM started_at) AS BIGINT) AS started_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM finished_at) AS BIGINT) AS finished_at_unix_secs
FROM request_candidates
ORDER BY created_at DESC
LIMIT $1
"#;

#[derive(Debug, Clone)]
pub struct SqlxRequestCandidateReadRepository {
    pool: PgPool,
}

impl SqlxRequestCandidateReadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn list_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        let rows = sqlx::query(LIST_BY_REQUEST_ID_SQL)
            .bind(request_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(map_request_candidate_row).collect()
    }

    pub async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(LIST_RECENT_SQL)
            .bind(i64::try_from(limit).map_err(|_| {
                DataLayerError::UnexpectedValue(format!(
                    "invalid recent request candidate limit: {limit}"
                ))
            })?)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(map_request_candidate_row).collect()
    }
}

#[async_trait]
impl RequestCandidateReadRepository for SqlxRequestCandidateReadRepository {
    async fn list_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        Self::list_by_request_id(self, request_id).await
    }

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        Self::list_recent(self, limit).await
    }
}

fn map_request_candidate_row(
    row: &sqlx::postgres::PgRow,
) -> Result<StoredRequestCandidate, DataLayerError> {
    let status =
        RequestCandidateStatus::from_database(row.try_get::<String, _>("status")?.as_str())?;
    StoredRequestCandidate::new(
        row.try_get("id")?,
        row.try_get("request_id")?,
        row.try_get("user_id")?,
        row.try_get("api_key_id")?,
        row.try_get("username")?,
        row.try_get("api_key_name")?,
        row.try_get("candidate_index")?,
        row.try_get("retry_index")?,
        row.try_get("provider_id")?,
        row.try_get("endpoint_id")?,
        row.try_get("key_id")?,
        status,
        row.try_get("skip_reason")?,
        row.try_get("is_cached")?,
        row.try_get("status_code")?,
        row.try_get("error_type")?,
        row.try_get("error_message")?,
        row.try_get("latency_ms")?,
        row.try_get("concurrent_requests")?,
        row.try_get("extra_data")?,
        row.try_get("required_capabilities")?,
        row.try_get("created_at_unix_secs")?,
        row.try_get("started_at_unix_secs")?,
        row.try_get("finished_at_unix_secs")?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxRequestCandidateReadRepository;
    use crate::postgres::{PostgresPoolConfig, PostgresPoolFactory};

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
        let repository = SqlxRequestCandidateReadRepository::new(pool);
        let _ = repository.pool();
    }
}
