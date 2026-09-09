use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

use aether_data_contracts::repository::provider_key_task_events::{
    ProviderKeyTaskEvent, ProviderKeyTaskEventQuery, ProviderKeyTaskEventReadRepository,
    ProviderKeyTaskEventWriteRepository,
};

use crate::{error::SqlxResultExt, DataLayerError};

#[derive(Debug, Clone)]
pub struct SqlxProviderKeyTaskEventRepository {
    pool: PgPool,
}

impl SqlxProviderKeyTaskEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProviderKeyTaskEventReadRepository for SqlxProviderKeyTaskEventRepository {
    async fn list_provider_key_task_events(
        &self,
        query: &ProviderKeyTaskEventQuery,
    ) -> Result<Vec<ProviderKeyTaskEvent>, DataLayerError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            r#"SELECT id, task_key, task_run_id, event_type, provider_id, provider_name,
provider_type, provider_api_key_id, provider_api_key_name, action, status, message,
reason, created_at_unix_secs
FROM provider_key_task_events WHERE task_key = "#,
        );
        builder.push_bind(&query.task_key);
        if let Some(run_id) = &query.task_run_id {
            builder.push(" AND task_run_id = ").push_bind(run_id);
        }
        if query.descending {
            builder.push(" ORDER BY created_at_unix_secs DESC, id DESC");
        } else {
            builder.push(" ORDER BY created_at_unix_secs ASC, id ASC");
        }
        if let Some(limit) = query.limit {
            builder
                .push(" LIMIT ")
                .push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
        }
        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_postgres_err()?;
        let mut events = Vec::with_capacity(rows.len());
        for row in &rows {
            events.push(map_event_row(row)?);
        }
        Ok(events)
    }
}

#[async_trait]
impl ProviderKeyTaskEventWriteRepository for SqlxProviderKeyTaskEventRepository {
    async fn append_provider_key_task_events(
        &self,
        events: &[ProviderKeyTaskEvent],
    ) -> Result<usize, DataLayerError> {
        if events.is_empty() {
            return Ok(0);
        }
        for event in events {
            to_i64(event.created_at_unix_secs, "created_at_unix_secs")?;
        }
        let mut builder = QueryBuilder::<Postgres>::new(
            r#"INSERT INTO provider_key_task_events (
id, task_key, task_run_id, event_type, provider_id, provider_name, provider_type,
provider_api_key_id, provider_api_key_name, action, status, message, reason,
created_at_unix_secs
) "#,
        );
        builder.push_values(events, |mut values, event| {
            values
                .push_bind(&event.id)
                .push_bind(&event.task_key)
                .push_bind(&event.task_run_id)
                .push_bind(&event.event_type)
                .push_bind(&event.provider_id)
                .push_bind(&event.provider_name)
                .push_bind(&event.provider_type)
                .push_bind(&event.provider_api_key_id)
                .push_bind(&event.provider_api_key_name)
                .push_bind(&event.action)
                .push_bind(&event.status)
                .push_bind(&event.message)
                .push_bind(&event.reason)
                .push_bind(event.created_at_unix_secs as i64);
        });
        builder.push(" ON CONFLICT (id) DO NOTHING");
        let result = builder
            .build()
            .execute(&self.pool)
            .await
            .map_postgres_err()?;
        Ok(result.rows_affected() as usize)
    }

    async fn delete_provider_key_task_events_before(
        &self,
        cutoff_unix_secs: u64,
    ) -> Result<usize, DataLayerError> {
        let cutoff = to_i64(cutoff_unix_secs, "cutoff_unix_secs")?;
        let result =
            sqlx::query("DELETE FROM provider_key_task_events WHERE created_at_unix_secs < $1")
                .bind(cutoff)
                .execute(&self.pool)
                .await
                .map_postgres_err()?;
        Ok(result.rows_affected() as usize)
    }
}

fn map_event_row(row: &sqlx::postgres::PgRow) -> Result<ProviderKeyTaskEvent, DataLayerError> {
    Ok(ProviderKeyTaskEvent {
        id: row.try_get("id").map_postgres_err()?,
        task_key: row.try_get("task_key").map_postgres_err()?,
        task_run_id: row.try_get("task_run_id").map_postgres_err()?,
        event_type: row.try_get("event_type").map_postgres_err()?,
        provider_id: row.try_get("provider_id").map_postgres_err()?,
        provider_name: row.try_get("provider_name").map_postgres_err()?,
        provider_type: row.try_get("provider_type").map_postgres_err()?,
        provider_api_key_id: row.try_get("provider_api_key_id").map_postgres_err()?,
        provider_api_key_name: row.try_get("provider_api_key_name").map_postgres_err()?,
        action: row.try_get("action").map_postgres_err()?,
        status: row.try_get("status").map_postgres_err()?,
        message: row.try_get("message").map_postgres_err()?,
        reason: row.try_get("reason").map_postgres_err()?,
        created_at_unix_secs: postgres_u64(row, "created_at_unix_secs")?,
    })
}

fn postgres_u64(row: &sqlx::postgres::PgRow, column: &str) -> Result<u64, DataLayerError> {
    let value: i64 = row.try_get(column).map_postgres_err()?;
    u64::try_from(value)
        .map_err(|_| DataLayerError::UnexpectedValue(format!("{column} is negative")))
}

fn to_i64(value: u64, field: &str) -> Result<i64, DataLayerError> {
    i64::try_from(value).map_err(|_| DataLayerError::InvalidInput(format!("{field} overflow")))
}

#[cfg(test)]
mod tests {
    use super::SqlxProviderKeyTaskEventRepository;
    use crate::{PostgresPoolConfig, PostgresPoolFactory};

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
        let _repository = SqlxProviderKeyTaskEventRepository::new(pool);
    }
}
