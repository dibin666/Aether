use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use std::collections::BTreeMap;

use aether_data_contracts::repository::quota::{
    ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery,
    ProviderKeyQuotaWindowObservation, ProviderQuotaReadRepository, ProviderQuotaWriteRepository,
    StoredProviderQuotaSnapshot,
};
use aether_data_query::{DialectSql, SelectColumn, SelectQuery, SqlDialect};

use crate::{error::SqlxResultExt, DataLayerError};

fn quota_snapshot_select() -> SelectQuery<'static> {
    SelectQuery::new("providers").select_columns([
        SelectColumn::expr("id").alias("provider_id"),
        SelectColumn::expr(
            DialectSql::common("billing_type").with_postgres("CAST(billing_type AS TEXT)"),
        )
        .alias("billing_type"),
        SelectColumn::expr(DialectSql::common(
            "CAST(monthly_quota_usd AS DOUBLE PRECISION)",
        ))
        .alias("monthly_quota_usd"),
        SelectColumn::expr(DialectSql::common(
            "CAST(COALESCE(monthly_used_usd, 0) AS DOUBLE PRECISION)",
        ))
        .alias("monthly_used_usd"),
        SelectColumn::expr("quota_reset_day"),
        SelectColumn::expr(DialectSql::common(
            "CAST(EXTRACT(EPOCH FROM quota_last_reset_at) AS BIGINT)",
        ))
        .alias("quota_last_reset_at_unix_secs"),
        SelectColumn::expr(DialectSql::common(
            "CAST(EXTRACT(EPOCH FROM quota_expires_at) AS BIGINT)",
        ))
        .alias("quota_expires_at_unix_secs"),
        SelectColumn::expr("is_active"),
    ])
}

const RESET_DUE_SQL: &str = r#"
UPDATE providers
SET
  monthly_used_usd = 0,
  quota_last_reset_at = TO_TIMESTAMP($1::double precision),
  updated_at = NOW()
WHERE
  billing_type = 'monthly_quota'
  AND is_active = TRUE
  AND (
    quota_last_reset_at IS NULL
    OR (EXTRACT(EPOCH FROM TO_TIMESTAMP($1::double precision)) - EXTRACT(EPOCH FROM quota_last_reset_at)) >= (quota_reset_day * 86400)
  )
"#;

#[derive(Debug, Clone)]
pub struct SqlxProviderQuotaRepository {
    pool: PgPool,
}

impl SqlxProviderQuotaRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProviderQuotaReadRepository for SqlxProviderQuotaRepository {
    async fn find_by_provider_id(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, DataLayerError> {
        let mut statement = quota_snapshot_select().statement::<Postgres>(SqlDialect::Postgres);
        statement.where_eq("id", provider_id.to_string()).limit(1);
        let row = statement
            .finish()
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_postgres_err()?;
        row.as_ref().map(map_row).transpose()
    }

    async fn find_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderQuotaSnapshot>, DataLayerError> {
        if provider_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut statement = quota_snapshot_select().statement::<Postgres>(SqlDialect::Postgres);
        statement
            .where_in("id", provider_ids)
            .order_by_sql("id ASC");
        statement
            .finish()
            .build()
            .fetch_all(&self.pool)
            .await
            .map_postgres_err()?
            .iter()
            .map(map_row)
            .collect()
    }

    async fn list_key_quota_observations(
        &self,
        query: &ProviderKeyQuotaObservationQuery,
    ) -> Result<Vec<ProviderKeyQuotaObservation>, DataLayerError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            r#"SELECT provider_api_key_id, provider_id, provider_api_key_name, provider_type,
bucket_start_unix_secs, observed_at_unix_secs, source, plan_type, status_code, status_label,
freshness, credits_balance, credits_unlimited, reset_credits_count
FROM provider_key_quota_observations WHERE provider_id = "#,
        );
        builder.push_bind(&query.provider_id);
        if let Some(key_id) = &query.provider_api_key_id {
            builder
                .push(" AND provider_api_key_id = ")
                .push_bind(key_id);
        }
        if let Some(from) = query.observed_from_unix_secs {
            builder
                .push(" AND observed_at_unix_secs >= ")
                .push_bind(to_i64(from, "quota observation from")?);
        }
        if let Some(until) = query.observed_until_unix_secs {
            builder
                .push(" AND observed_at_unix_secs < ")
                .push_bind(to_i64(until, "quota observation until")?);
        }
        builder.push(" ORDER BY observed_at_unix_secs DESC");
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
        let identities = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>("provider_api_key_id")
                        .map_postgres_err()?,
                    postgres_u64(row, "bucket_start_unix_secs")?,
                ))
            })
            .collect::<Result<Vec<_>, DataLayerError>>()?;
        let mut windows_by_observation = load_postgres_windows(&self.pool, &identities).await?;
        let mut observations = Vec::with_capacity(rows.len());
        for row in rows {
            let key_id: String = row.try_get("provider_api_key_id").map_postgres_err()?;
            let bucket = postgres_u64(&row, "bucket_start_unix_secs")?;
            let windows = windows_by_observation
                .remove(&(key_id, bucket))
                .unwrap_or_default();
            observations.push(map_observation_row(&row, windows)?);
        }
        Ok(observations)
    }
}

#[async_trait]
impl ProviderQuotaWriteRepository for SqlxProviderQuotaRepository {
    async fn reset_due(&self, now_unix_secs: u64) -> Result<usize, DataLayerError> {
        let result = sqlx::query(RESET_DUE_SQL)
            .bind(i64::try_from(now_unix_secs).map_err(|_| {
                DataLayerError::InvalidInput("provider quota reset timestamp overflow".to_string())
            })?)
            .execute(&self.pool)
            .await
            .map_postgres_err()?;
        Ok(result.rows_affected() as usize)
    }

    async fn upsert_key_quota_observation(
        &self,
        observation: &ProviderKeyQuotaObservation,
    ) -> Result<bool, DataLayerError> {
        let mut transaction = self.pool.begin().await.map_postgres_err()?;
        let changed = sqlx::query(
            r#"INSERT INTO provider_key_quota_observations (
provider_api_key_id, provider_id, provider_api_key_name, provider_type,
bucket_start_unix_secs, observed_at_unix_secs, source, plan_type, status_code, status_label,
freshness, credits_balance, credits_unlimited, reset_credits_count
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
ON CONFLICT(provider_api_key_id, bucket_start_unix_secs) DO UPDATE SET
provider_id = EXCLUDED.provider_id,
provider_api_key_name = EXCLUDED.provider_api_key_name,
provider_type = EXCLUDED.provider_type,
observed_at_unix_secs = EXCLUDED.observed_at_unix_secs,
source = EXCLUDED.source,
plan_type = EXCLUDED.plan_type,
status_code = EXCLUDED.status_code,
status_label = EXCLUDED.status_label,
freshness = EXCLUDED.freshness,
credits_balance = EXCLUDED.credits_balance,
credits_unlimited = EXCLUDED.credits_unlimited,
reset_credits_count = EXCLUDED.reset_credits_count
WHERE EXCLUDED.observed_at_unix_secs > provider_key_quota_observations.observed_at_unix_secs"#,
        )
        .bind(&observation.provider_api_key_id)
        .bind(&observation.provider_id)
        .bind(&observation.provider_api_key_name)
        .bind(&observation.provider_type)
        .bind(to_i64(observation.bucket_start_unix_secs, "quota bucket")?)
        .bind(to_i64(
            observation.observed_at_unix_secs,
            "quota observed_at",
        )?)
        .bind(&observation.source)
        .bind(&observation.plan_type)
        .bind(&observation.status_code)
        .bind(&observation.status_label)
        .bind(&observation.freshness)
        .bind(observation.credits_balance)
        .bind(observation.credits_unlimited)
        .bind(to_i64(
            observation.reset_credits_count,
            "quota reset credits",
        )?)
        .execute(&mut *transaction)
        .await
        .map_postgres_err()?
        .rows_affected()
            > 0;
        if changed {
            sqlx::query("DELETE FROM provider_key_quota_window_observations WHERE provider_api_key_id = $1 AND bucket_start_unix_secs = $2")
                .bind(&observation.provider_api_key_id)
                .bind(to_i64(observation.bucket_start_unix_secs, "quota bucket")?)
                .execute(&mut *transaction)
                .await
                .map_postgres_err()?;
            for window in &observation.windows {
                insert_postgres_window(&mut transaction, observation, window).await?;
            }
        }
        transaction.commit().await.map_postgres_err()?;
        Ok(changed)
    }
}

async fn insert_postgres_window(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    observation: &ProviderKeyQuotaObservation,
    window: &ProviderKeyQuotaWindowObservation,
) -> Result<(), DataLayerError> {
    sqlx::query(
        r#"INSERT INTO provider_key_quota_window_observations (
provider_api_key_id, bucket_start_unix_secs, window_identity, code, label, scope, model, unit,
used_percent, remaining_percent, used_value, remaining_value, limit_value, reset_at_unix_secs,
window_minutes, exhausted, local_request_count, local_total_tokens, local_cost_usd
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
    )
    .bind(&observation.provider_api_key_id)
    .bind(to_i64(observation.bucket_start_unix_secs, "quota bucket")?)
    .bind(&window.window_identity)
    .bind(&window.code)
    .bind(&window.label)
    .bind(&window.scope)
    .bind(&window.model)
    .bind(&window.unit)
    .bind(window.used_percent)
    .bind(window.remaining_percent)
    .bind(window.used_value)
    .bind(window.remaining_value)
    .bind(window.limit_value)
    .bind(
        window
            .reset_at_unix_secs
            .map(|value| to_i64(value, "quota reset_at"))
            .transpose()?,
    )
    .bind(
        window
            .window_minutes
            .map(|value| to_i64(value, "quota window minutes"))
            .transpose()?,
    )
    .bind(window.exhausted)
    .bind(to_i64(window.local_request_count, "quota local requests")?)
    .bind(to_i64(window.local_total_tokens, "quota local tokens")?)
    .bind(window.local_cost_usd)
    .execute(&mut **transaction)
    .await
    .map_postgres_err()?;
    Ok(())
}

async fn load_postgres_windows(
    pool: &PgPool,
    identities: &[(String, u64)],
) -> Result<BTreeMap<(String, u64), Vec<ProviderKeyQuotaWindowObservation>>, DataLayerError> {
    let mut values = BTreeMap::new();
    for chunk in identities.chunks(1_000) {
        let mut builder = QueryBuilder::<Postgres>::new(
            r#"SELECT provider_api_key_id, bucket_start_unix_secs, window_identity, code, label,
scope, model, unit, used_percent, remaining_percent, used_value, remaining_value, limit_value,
reset_at_unix_secs, window_minutes, exhausted, local_request_count, local_total_tokens,
local_cost_usd FROM provider_key_quota_window_observations WHERE "#,
        );
        {
            let mut separated = builder.separated(" OR ");
            for (key_id, bucket) in chunk {
                separated
                    .push("(provider_api_key_id = ")
                    .push_bind_unseparated(key_id)
                    .push_unseparated(" AND bucket_start_unix_secs = ")
                    .push_bind_unseparated(to_i64(*bucket, "quota bucket")?)
                    .push_unseparated(")");
            }
        }
        builder.push(" ORDER BY provider_api_key_id, bucket_start_unix_secs, window_identity");
        for row in builder.build().fetch_all(pool).await.map_postgres_err()? {
            let key_id = row
                .try_get::<String, _>("provider_api_key_id")
                .map_postgres_err()?;
            let bucket = postgres_u64(&row, "bucket_start_unix_secs")?;
            values
                .entry((key_id, bucket))
                .or_insert_with(Vec::new)
                .push(map_window_row(&row)?);
        }
    }
    Ok(values)
}

fn map_observation_row(
    row: &sqlx::postgres::PgRow,
    windows: Vec<ProviderKeyQuotaWindowObservation>,
) -> Result<ProviderKeyQuotaObservation, DataLayerError> {
    Ok(ProviderKeyQuotaObservation {
        provider_api_key_id: row.try_get("provider_api_key_id").map_postgres_err()?,
        provider_id: row.try_get("provider_id").map_postgres_err()?,
        provider_api_key_name: row.try_get("provider_api_key_name").map_postgres_err()?,
        provider_type: row.try_get("provider_type").map_postgres_err()?,
        bucket_start_unix_secs: postgres_u64(row, "bucket_start_unix_secs")?,
        observed_at_unix_secs: postgres_u64(row, "observed_at_unix_secs")?,
        source: row.try_get("source").map_postgres_err()?,
        plan_type: row.try_get("plan_type").map_postgres_err()?,
        status_code: row.try_get("status_code").map_postgres_err()?,
        status_label: row.try_get("status_label").map_postgres_err()?,
        freshness: row.try_get("freshness").map_postgres_err()?,
        credits_balance: row.try_get("credits_balance").map_postgres_err()?,
        credits_unlimited: row.try_get("credits_unlimited").map_postgres_err()?,
        reset_credits_count: postgres_u64(row, "reset_credits_count")?,
        windows,
    })
}

fn map_window_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ProviderKeyQuotaWindowObservation, DataLayerError> {
    Ok(ProviderKeyQuotaWindowObservation {
        window_identity: row.try_get("window_identity").map_postgres_err()?,
        code: row.try_get("code").map_postgres_err()?,
        label: row.try_get("label").map_postgres_err()?,
        scope: row.try_get("scope").map_postgres_err()?,
        model: row.try_get("model").map_postgres_err()?,
        unit: row.try_get("unit").map_postgres_err()?,
        used_percent: row.try_get("used_percent").map_postgres_err()?,
        remaining_percent: row.try_get("remaining_percent").map_postgres_err()?,
        used_value: row.try_get("used_value").map_postgres_err()?,
        remaining_value: row.try_get("remaining_value").map_postgres_err()?,
        limit_value: row.try_get("limit_value").map_postgres_err()?,
        reset_at_unix_secs: postgres_optional_u64(row, "reset_at_unix_secs")?,
        window_minutes: postgres_optional_u64(row, "window_minutes")?,
        exhausted: row.try_get("exhausted").map_postgres_err()?,
        local_request_count: postgres_u64(row, "local_request_count")?,
        local_total_tokens: postgres_u64(row, "local_total_tokens")?,
        local_cost_usd: row.try_get("local_cost_usd").map_postgres_err()?,
    })
}

fn postgres_u64(row: &sqlx::postgres::PgRow, column: &str) -> Result<u64, DataLayerError> {
    let value: i64 = row.try_get(column).map_postgres_err()?;
    u64::try_from(value)
        .map_err(|_| DataLayerError::UnexpectedValue(format!("{column} is negative")))
}

fn postgres_optional_u64(
    row: &sqlx::postgres::PgRow,
    column: &str,
) -> Result<Option<u64>, DataLayerError> {
    let value: Option<i64> = row.try_get(column).map_postgres_err()?;
    value
        .map(|value| {
            u64::try_from(value)
                .map_err(|_| DataLayerError::UnexpectedValue(format!("{column} is negative")))
        })
        .transpose()
}

fn to_i64(value: u64, field: &str) -> Result<i64, DataLayerError> {
    i64::try_from(value).map_err(|_| DataLayerError::InvalidInput(format!("{field} overflow")))
}

fn map_row(row: &sqlx::postgres::PgRow) -> Result<StoredProviderQuotaSnapshot, DataLayerError> {
    StoredProviderQuotaSnapshot::new(
        row.try_get("provider_id").map_postgres_err()?,
        row.try_get("billing_type").map_postgres_err()?,
        row.try_get("monthly_quota_usd").map_postgres_err()?,
        row.try_get("monthly_used_usd").map_postgres_err()?,
        row.try_get("quota_reset_day").map_postgres_err()?,
        row.try_get("quota_last_reset_at_unix_secs")
            .map_postgres_err()?,
        row.try_get("quota_expires_at_unix_secs")
            .map_postgres_err()?,
        row.try_get("is_active").map_postgres_err()?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxProviderQuotaRepository;
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
        let _repository = SqlxProviderQuotaRepository::new(pool);
    }
}
