use async_trait::async_trait;
use sqlx::{mysql::MySqlRow, MySql, QueryBuilder, Row};
use std::collections::BTreeMap;

use aether_data_contracts::repository::quota::{
    ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery,
    ProviderKeyQuotaWindowObservation, ProviderQuotaReadRepository, ProviderQuotaWriteRepository,
    StoredProviderQuotaSnapshot,
};
use aether_data_query::{DialectSql, SelectColumn, SelectQuery, SqlDialect};

use crate::error::SqlResultExt;
use crate::{DataLayerError, MysqlPool};

fn quota_snapshot_select() -> SelectQuery<'static> {
    SelectQuery::new("providers").select_columns([
        SelectColumn::expr("id").alias("provider_id"),
        SelectColumn::expr(
            DialectSql::common("billing_type").with_postgres("CAST(billing_type AS TEXT)"),
        )
        .alias("billing_type"),
        SelectColumn::expr(
            DialectSql::dialect(
                "CAST(monthly_quota_usd AS DOUBLE PRECISION)",
                "CAST(monthly_quota_usd AS REAL)",
            )
            .with_mysql("CAST(monthly_quota_usd AS DOUBLE)"),
        )
        .alias("monthly_quota_usd"),
        SelectColumn::expr(
            DialectSql::dialect(
                "CAST(COALESCE(monthly_used_usd, 0) AS DOUBLE PRECISION)",
                "CAST(COALESCE(monthly_used_usd, 0) AS REAL)",
            )
            .with_mysql("CAST(COALESCE(monthly_used_usd, 0) AS DOUBLE)"),
        )
        .alias("monthly_used_usd"),
        SelectColumn::expr("quota_reset_day"),
        SelectColumn::expr(
            DialectSql::dialect(
                "CAST(EXTRACT(EPOCH FROM quota_last_reset_at) AS BIGINT)",
                "quota_last_reset_at",
            )
            .with_mysql("quota_last_reset_at"),
        )
        .alias("quota_last_reset_at_unix_secs"),
        SelectColumn::expr(
            DialectSql::dialect(
                "CAST(EXTRACT(EPOCH FROM quota_expires_at) AS BIGINT)",
                "quota_expires_at",
            )
            .with_mysql("quota_expires_at"),
        )
        .alias("quota_expires_at_unix_secs"),
        SelectColumn::expr("is_active"),
    ])
}

#[derive(Debug, Clone)]
pub struct MysqlProviderQuotaRepository {
    pool: MysqlPool,
}

impl MysqlProviderQuotaRepository {
    pub fn new(pool: MysqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProviderQuotaReadRepository for MysqlProviderQuotaRepository {
    async fn find_by_provider_id(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, DataLayerError> {
        let mut statement = quota_snapshot_select().statement::<MySql>(SqlDialect::MySql);
        statement.where_eq("id", provider_id.to_string()).limit(1);
        let row = statement
            .finish()
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_sql_err()?;
        row.as_ref().map(map_row).transpose()
    }

    async fn find_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderQuotaSnapshot>, DataLayerError> {
        if provider_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut statement = quota_snapshot_select().statement::<MySql>(SqlDialect::MySql);
        statement
            .where_in("id", provider_ids)
            .order_by_sql("id ASC");
        let rows = statement
            .finish()
            .build()
            .fetch_all(&self.pool)
            .await
            .map_sql_err()?;
        rows.iter().map(map_row).collect()
    }

    async fn list_key_quota_observations(
        &self,
        query: &ProviderKeyQuotaObservationQuery,
    ) -> Result<Vec<ProviderKeyQuotaObservation>, DataLayerError> {
        let mut builder = QueryBuilder::<MySql>::new(
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
                .push_bind(from);
        }
        if let Some(until) = query.observed_until_unix_secs {
            builder
                .push(" AND observed_at_unix_secs < ")
                .push_bind(until);
        }
        builder.push(" ORDER BY observed_at_unix_secs DESC");
        if let Some(limit) = query.limit {
            builder
                .push(" LIMIT ")
                .push_bind(u64::try_from(limit).unwrap_or(u64::MAX));
        }
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        let identities = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>("provider_api_key_id")
                        .map_sql_err()?,
                    row.try_get::<u64, _>("bucket_start_unix_secs")
                        .map_sql_err()?,
                ))
            })
            .collect::<Result<Vec<_>, DataLayerError>>()?;
        let mut windows_by_observation = load_mysql_windows(&self.pool, &identities).await?;
        let mut observations = Vec::with_capacity(rows.len());
        for row in rows {
            let key_id: String = row.try_get("provider_api_key_id").map_sql_err()?;
            let bucket: u64 = row.try_get("bucket_start_unix_secs").map_sql_err()?;
            let windows = windows_by_observation
                .remove(&(key_id, bucket))
                .unwrap_or_default();
            observations.push(map_observation_row(&row, windows)?);
        }
        Ok(observations)
    }
}

#[async_trait]
impl ProviderQuotaWriteRepository for MysqlProviderQuotaRepository {
    async fn reset_due(&self, now_unix_secs: u64) -> Result<usize, DataLayerError> {
        let now = i64::try_from(now_unix_secs).map_err(|_| {
            DataLayerError::InvalidInput("provider quota reset timestamp overflow".to_string())
        })?;
        let rows_affected = sqlx::query(
            r#"
UPDATE providers
SET monthly_used_usd = 0,
    quota_last_reset_at = ?,
    updated_at = ?
WHERE billing_type = 'monthly_quota'
  AND is_active = 1
  AND (
    quota_last_reset_at IS NULL
    OR (? - quota_last_reset_at) >= (quota_reset_day * 86400)
  )
"#,
        )
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_sql_err()?
        .rows_affected();
        Ok(usize::try_from(rows_affected).unwrap_or_default())
    }

    async fn upsert_key_quota_observation(
        &self,
        observation: &ProviderKeyQuotaObservation,
    ) -> Result<bool, DataLayerError> {
        let mut transaction = self.pool.begin().await.map_sql_err()?;
        let current = sqlx::query(
            "SELECT observed_at_unix_secs FROM provider_key_quota_observations WHERE provider_api_key_id = ? AND bucket_start_unix_secs = ? FOR UPDATE",
        )
        .bind(&observation.provider_api_key_id)
        .bind(observation.bucket_start_unix_secs)
        .fetch_optional(&mut *transaction)
        .await
        .map_sql_err()?;
        if current
            .as_ref()
            .map(|row| row.try_get::<u64, _>("observed_at_unix_secs"))
            .transpose()
            .map_sql_err()?
            .is_some_and(|current| current >= observation.observed_at_unix_secs)
        {
            transaction.commit().await.map_sql_err()?;
            return Ok(false);
        }

        sqlx::query(
            r#"INSERT INTO provider_key_quota_observations (
provider_api_key_id, provider_id, provider_api_key_name, provider_type,
bucket_start_unix_secs, observed_at_unix_secs, source, plan_type, status_code, status_label,
freshness, credits_balance, credits_unlimited, reset_credits_count
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON DUPLICATE KEY UPDATE
provider_id = VALUES(provider_id),
provider_api_key_name = VALUES(provider_api_key_name),
provider_type = VALUES(provider_type),
observed_at_unix_secs = VALUES(observed_at_unix_secs),
source = VALUES(source),
plan_type = VALUES(plan_type),
status_code = VALUES(status_code),
status_label = VALUES(status_label),
freshness = VALUES(freshness),
credits_balance = VALUES(credits_balance),
credits_unlimited = VALUES(credits_unlimited),
reset_credits_count = VALUES(reset_credits_count)"#,
        )
        .bind(&observation.provider_api_key_id)
        .bind(&observation.provider_id)
        .bind(&observation.provider_api_key_name)
        .bind(&observation.provider_type)
        .bind(observation.bucket_start_unix_secs)
        .bind(observation.observed_at_unix_secs)
        .bind(&observation.source)
        .bind(&observation.plan_type)
        .bind(&observation.status_code)
        .bind(&observation.status_label)
        .bind(&observation.freshness)
        .bind(observation.credits_balance)
        .bind(observation.credits_unlimited)
        .bind(observation.reset_credits_count)
        .execute(&mut *transaction)
        .await
        .map_sql_err()?;
        sqlx::query("DELETE FROM provider_key_quota_window_observations WHERE provider_api_key_id = ? AND bucket_start_unix_secs = ?")
            .bind(&observation.provider_api_key_id)
            .bind(observation.bucket_start_unix_secs)
            .execute(&mut *transaction)
            .await
            .map_sql_err()?;
        for window in &observation.windows {
            insert_mysql_window(&mut transaction, observation, window).await?;
        }
        transaction.commit().await.map_sql_err()?;
        Ok(true)
    }
}

async fn insert_mysql_window(
    transaction: &mut sqlx::Transaction<'_, MySql>,
    observation: &ProviderKeyQuotaObservation,
    window: &ProviderKeyQuotaWindowObservation,
) -> Result<(), DataLayerError> {
    sqlx::query(
        r#"INSERT INTO provider_key_quota_window_observations (
provider_api_key_id, bucket_start_unix_secs, window_identity, code, label, scope, model, unit,
used_percent, remaining_percent, used_value, remaining_value, limit_value, reset_at_unix_secs,
window_minutes, exhausted, local_request_count, local_total_tokens, local_cost_usd
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&observation.provider_api_key_id)
    .bind(observation.bucket_start_unix_secs)
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
    .bind(window.reset_at_unix_secs)
    .bind(window.window_minutes)
    .bind(window.exhausted)
    .bind(window.local_request_count)
    .bind(window.local_total_tokens)
    .bind(window.local_cost_usd)
    .execute(&mut **transaction)
    .await
    .map_sql_err()?;
    Ok(())
}

async fn load_mysql_windows(
    pool: &MysqlPool,
    identities: &[(String, u64)],
) -> Result<BTreeMap<(String, u64), Vec<ProviderKeyQuotaWindowObservation>>, DataLayerError> {
    let mut values = BTreeMap::new();
    for chunk in identities.chunks(500) {
        let mut builder = QueryBuilder::<MySql>::new(
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
                    .push_bind_unseparated(*bucket)
                    .push_unseparated(")");
            }
        }
        builder.push(" ORDER BY provider_api_key_id, bucket_start_unix_secs, window_identity");
        for row in builder.build().fetch_all(pool).await.map_sql_err()? {
            let key_id = row
                .try_get::<String, _>("provider_api_key_id")
                .map_sql_err()?;
            let bucket = row
                .try_get::<u64, _>("bucket_start_unix_secs")
                .map_sql_err()?;
            values
                .entry((key_id, bucket))
                .or_insert_with(Vec::new)
                .push(map_window_row(&row)?);
        }
    }
    Ok(values)
}

fn map_observation_row(
    row: &MySqlRow,
    windows: Vec<ProviderKeyQuotaWindowObservation>,
) -> Result<ProviderKeyQuotaObservation, DataLayerError> {
    Ok(ProviderKeyQuotaObservation {
        provider_api_key_id: row.try_get("provider_api_key_id").map_sql_err()?,
        provider_id: row.try_get("provider_id").map_sql_err()?,
        provider_api_key_name: row.try_get("provider_api_key_name").map_sql_err()?,
        provider_type: row.try_get("provider_type").map_sql_err()?,
        bucket_start_unix_secs: row.try_get("bucket_start_unix_secs").map_sql_err()?,
        observed_at_unix_secs: row.try_get("observed_at_unix_secs").map_sql_err()?,
        source: row.try_get("source").map_sql_err()?,
        plan_type: row.try_get("plan_type").map_sql_err()?,
        status_code: row.try_get("status_code").map_sql_err()?,
        status_label: row.try_get("status_label").map_sql_err()?,
        freshness: row.try_get("freshness").map_sql_err()?,
        credits_balance: row.try_get("credits_balance").map_sql_err()?,
        credits_unlimited: row.try_get("credits_unlimited").map_sql_err()?,
        reset_credits_count: row.try_get("reset_credits_count").map_sql_err()?,
        windows,
    })
}

fn map_window_row(row: &MySqlRow) -> Result<ProviderKeyQuotaWindowObservation, DataLayerError> {
    Ok(ProviderKeyQuotaWindowObservation {
        window_identity: row.try_get("window_identity").map_sql_err()?,
        code: row.try_get("code").map_sql_err()?,
        label: row.try_get("label").map_sql_err()?,
        scope: row.try_get("scope").map_sql_err()?,
        model: row.try_get("model").map_sql_err()?,
        unit: row.try_get("unit").map_sql_err()?,
        used_percent: row.try_get("used_percent").map_sql_err()?,
        remaining_percent: row.try_get("remaining_percent").map_sql_err()?,
        used_value: row.try_get("used_value").map_sql_err()?,
        remaining_value: row.try_get("remaining_value").map_sql_err()?,
        limit_value: row.try_get("limit_value").map_sql_err()?,
        reset_at_unix_secs: row.try_get("reset_at_unix_secs").map_sql_err()?,
        window_minutes: row.try_get("window_minutes").map_sql_err()?,
        exhausted: row.try_get("exhausted").map_sql_err()?,
        local_request_count: row.try_get("local_request_count").map_sql_err()?,
        local_total_tokens: row.try_get("local_total_tokens").map_sql_err()?,
        local_cost_usd: row.try_get("local_cost_usd").map_sql_err()?,
    })
}

fn map_row(row: &MySqlRow) -> Result<StoredProviderQuotaSnapshot, DataLayerError> {
    StoredProviderQuotaSnapshot::new(
        row.try_get("provider_id").map_sql_err()?,
        row.try_get("billing_type").map_sql_err()?,
        row.try_get("monthly_quota_usd").map_sql_err()?,
        row.try_get("monthly_used_usd").map_sql_err()?,
        row.try_get("quota_reset_day").map_sql_err()?,
        row.try_get("quota_last_reset_at_unix_secs").map_sql_err()?,
        row.try_get("quota_expires_at_unix_secs").map_sql_err()?,
        row.try_get("is_active").map_sql_err()?,
    )
}

#[cfg(test)]
mod tests {
    use super::{quota_snapshot_select, MysqlProviderQuotaRepository};
    use aether_data_query::SqlDialect;

    #[test]
    fn quota_projection_renders_for_mysql() {
        let sql = quota_snapshot_select().render(SqlDialect::MySql);

        assert!(sql.contains("id AS `provider_id`"));
        assert!(sql.contains("CAST(monthly_quota_usd AS DOUBLE) AS `monthly_quota_usd`"));
        assert!(sql.contains("quota_last_reset_at AS `quota_last_reset_at_unix_secs`"));
    }

    #[tokio::test]
    async fn repository_builds_from_lazy_pool() {
        let pool = sqlx::mysql::MySqlPoolOptions::new().connect_lazy_with(
            "mysql://user:pass@localhost:3306/aether"
                .parse()
                .expect("mysql options should parse"),
        );

        let _repository = MysqlProviderQuotaRepository::new(pool);
    }
}
