use async_trait::async_trait;
use sqlx::{PgPool, Row};

use super::types::{StoredRequestUsageAudit, UsageReadRepository};
use crate::DataLayerError;

const FIND_BY_REQUEST_ID_SQL: &str = r#"
SELECT
  id,
  request_id,
  user_id,
  api_key_id,
  username,
  api_key_name,
  provider_name,
  model,
  target_model,
  provider_id,
  provider_endpoint_id,
  provider_api_key_id,
  request_type,
  api_format,
  api_family,
  endpoint_kind,
  endpoint_api_format,
  provider_api_family,
  provider_endpoint_kind,
  COALESCE(has_format_conversion, FALSE) AS has_format_conversion,
  COALESCE(is_stream, FALSE) AS is_stream,
  input_tokens,
  output_tokens,
  total_tokens,
  COALESCE(CAST(total_cost_usd AS DOUBLE PRECISION), 0) AS total_cost_usd,
  COALESCE(CAST(actual_total_cost_usd AS DOUBLE PRECISION), 0) AS actual_total_cost_usd,
  status_code,
  error_message,
  error_category,
  response_time_ms,
  first_byte_time_ms,
  status,
  billing_status,
  CAST(EXTRACT(EPOCH FROM created_at) AS BIGINT) AS created_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM updated_at) AS BIGINT) AS updated_at_unix_secs,
  CAST(EXTRACT(EPOCH FROM finalized_at) AS BIGINT) AS finalized_at_unix_secs
FROM "usage"
WHERE request_id = $1
LIMIT 1
"#;

#[derive(Debug, Clone)]
pub struct SqlxUsageReadRepository {
    pool: PgPool,
}

impl SqlxUsageReadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<StoredRequestUsageAudit>, DataLayerError> {
        let row = sqlx::query(FIND_BY_REQUEST_ID_SQL)
            .bind(request_id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(map_usage_row).transpose()
    }
}

#[async_trait]
impl UsageReadRepository for SqlxUsageReadRepository {
    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<StoredRequestUsageAudit>, DataLayerError> {
        Self::find_by_request_id(self, request_id).await
    }
}

fn map_usage_row(row: &sqlx::postgres::PgRow) -> Result<StoredRequestUsageAudit, DataLayerError> {
    StoredRequestUsageAudit::new(
        row.try_get("id")?,
        row.try_get("request_id")?,
        row.try_get("user_id")?,
        row.try_get("api_key_id")?,
        row.try_get("username")?,
        row.try_get("api_key_name")?,
        row.try_get("provider_name")?,
        row.try_get("model")?,
        row.try_get("target_model")?,
        row.try_get("provider_id")?,
        row.try_get("provider_endpoint_id")?,
        row.try_get("provider_api_key_id")?,
        row.try_get("request_type")?,
        row.try_get("api_format")?,
        row.try_get("api_family")?,
        row.try_get("endpoint_kind")?,
        row.try_get("endpoint_api_format")?,
        row.try_get("provider_api_family")?,
        row.try_get("provider_endpoint_kind")?,
        row.try_get("has_format_conversion")?,
        row.try_get("is_stream")?,
        row.try_get("input_tokens")?,
        row.try_get("output_tokens")?,
        row.try_get("total_tokens")?,
        row.try_get("total_cost_usd")?,
        row.try_get("actual_total_cost_usd")?,
        row.try_get("status_code")?,
        row.try_get("error_message")?,
        row.try_get("error_category")?,
        row.try_get("response_time_ms")?,
        row.try_get("first_byte_time_ms")?,
        row.try_get("status")?,
        row.try_get("billing_status")?,
        row.try_get("created_at_unix_secs")?,
        row.try_get("updated_at_unix_secs")?,
        row.try_get("finalized_at_unix_secs")?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxUsageReadRepository;
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
        let repository = SqlxUsageReadRepository::new(pool);
        let _ = repository.pool();
    }
}
