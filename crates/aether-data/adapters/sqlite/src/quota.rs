use async_trait::async_trait;
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite};
use std::collections::BTreeMap;

use aether_data_contracts::repository::quota::{
    ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery,
    ProviderKeyQuotaWindowObservation, ProviderQuotaReadRepository, ProviderQuotaWriteRepository,
    StoredProviderQuotaSnapshot,
};
use aether_data_query::{DialectSql, SelectColumn, SelectQuery, SqlDialect};

use crate::error::SqlResultExt;
use crate::{sqlite_optional_real, sqlite_real, DataLayerError, SqlitePool};

fn quota_snapshot_select() -> SelectQuery<'static> {
    SelectQuery::new("providers").select_columns([
        SelectColumn::expr("id").alias("provider_id"),
        SelectColumn::expr(
            DialectSql::common("billing_type").with_postgres("CAST(billing_type AS TEXT)"),
        )
        .alias("billing_type"),
        SelectColumn::expr(DialectSql::dialect(
            "CAST(monthly_quota_usd AS DOUBLE PRECISION)",
            "CAST(monthly_quota_usd AS REAL)",
        ))
        .alias("monthly_quota_usd"),
        SelectColumn::expr(DialectSql::dialect(
            "CAST(COALESCE(monthly_used_usd, 0) AS DOUBLE PRECISION)",
            "CAST(COALESCE(monthly_used_usd, 0) AS REAL)",
        ))
        .alias("monthly_used_usd"),
        SelectColumn::expr("quota_reset_day"),
        SelectColumn::expr(DialectSql::dialect(
            "CAST(EXTRACT(EPOCH FROM quota_last_reset_at) AS BIGINT)",
            "quota_last_reset_at",
        ))
        .alias("quota_last_reset_at_unix_secs"),
        SelectColumn::expr(DialectSql::dialect(
            "CAST(EXTRACT(EPOCH FROM quota_expires_at) AS BIGINT)",
            "quota_expires_at",
        ))
        .alias("quota_expires_at_unix_secs"),
        SelectColumn::expr("is_active"),
    ])
}

#[derive(Debug, Clone)]
pub struct SqliteProviderQuotaRepository {
    pool: SqlitePool,
}

impl SqliteProviderQuotaRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProviderQuotaReadRepository for SqliteProviderQuotaRepository {
    async fn find_by_provider_id(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, DataLayerError> {
        let mut statement = quota_snapshot_select().statement::<Sqlite>(SqlDialect::Sqlite);
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

        let mut statement = quota_snapshot_select().statement::<Sqlite>(SqlDialect::Sqlite);
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
        let mut builder = QueryBuilder::<Sqlite>::new(
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
        let rows = builder.build().fetch_all(&self.pool).await.map_sql_err()?;
        let identities = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String, _>("provider_api_key_id")
                        .map_sql_err()?,
                    sqlite_u64(row, "bucket_start_unix_secs")?,
                ))
            })
            .collect::<Result<Vec<_>, DataLayerError>>()?;
        let mut windows_by_observation = load_sqlite_windows(&self.pool, &identities).await?;
        let mut observations = Vec::with_capacity(rows.len());
        for row in rows {
            let key_id: String = row.try_get("provider_api_key_id").map_sql_err()?;
            let bucket = sqlite_u64(&row, "bucket_start_unix_secs")?;
            let windows = windows_by_observation
                .remove(&(key_id, bucket))
                .unwrap_or_default();
            observations.push(map_observation_row(&row, windows)?);
        }
        Ok(observations)
    }
}

#[async_trait]
impl ProviderQuotaWriteRepository for SqliteProviderQuotaRepository {
    async fn reset_due(&self, now_unix_secs: u64) -> Result<usize, DataLayerError> {
        let now = i64::try_from(now_unix_secs).map_err(|_| {
            DataLayerError::InvalidInput("provider quota reset timestamp overflow".to_string())
        })?;
        let rows_affected = sqlx::query(
            r#"
UPDATE providers
SET monthly_used_usd = 0.0,
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
        let changed = sqlx::query(
            r#"INSERT INTO provider_key_quota_observations (
provider_api_key_id, provider_id, provider_api_key_name, provider_type,
bucket_start_unix_secs, observed_at_unix_secs, source, plan_type, status_code, status_label,
freshness, credits_balance, credits_unlimited, reset_credits_count
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(provider_api_key_id, bucket_start_unix_secs) DO UPDATE SET
provider_id = excluded.provider_id,
provider_api_key_name = excluded.provider_api_key_name,
provider_type = excluded.provider_type,
observed_at_unix_secs = excluded.observed_at_unix_secs,
source = excluded.source,
plan_type = excluded.plan_type,
status_code = excluded.status_code,
status_label = excluded.status_label,
freshness = excluded.freshness,
credits_balance = excluded.credits_balance,
credits_unlimited = excluded.credits_unlimited,
reset_credits_count = excluded.reset_credits_count
WHERE excluded.observed_at_unix_secs > provider_key_quota_observations.observed_at_unix_secs"#,
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
        .map_sql_err()?
        .rows_affected()
            > 0;
        if changed {
            sqlx::query(
                "DELETE FROM provider_key_quota_window_observations WHERE provider_api_key_id = ? AND bucket_start_unix_secs = ?",
            )
            .bind(&observation.provider_api_key_id)
            .bind(to_i64(observation.bucket_start_unix_secs, "quota bucket")?)
            .execute(&mut *transaction)
            .await
            .map_sql_err()?;
            for window in &observation.windows {
                insert_sqlite_window(&mut transaction, observation, window).await?;
            }
        }
        transaction.commit().await.map_sql_err()?;
        Ok(changed)
    }
}

async fn insert_sqlite_window(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
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
    .map_sql_err()?;
    Ok(())
}

async fn load_sqlite_windows(
    pool: &SqlitePool,
    identities: &[(String, u64)],
) -> Result<BTreeMap<(String, u64), Vec<ProviderKeyQuotaWindowObservation>>, DataLayerError> {
    let mut values = BTreeMap::new();
    for chunk in identities.chunks(400) {
        let mut builder = QueryBuilder::<Sqlite>::new(
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
        for row in builder.build().fetch_all(pool).await.map_sql_err()? {
            let key_id = row
                .try_get::<String, _>("provider_api_key_id")
                .map_sql_err()?;
            let bucket = sqlite_u64(&row, "bucket_start_unix_secs")?;
            values
                .entry((key_id, bucket))
                .or_insert_with(Vec::new)
                .push(map_window_row(&row)?);
        }
    }
    Ok(values)
}

fn map_observation_row(
    row: &SqliteRow,
    windows: Vec<ProviderKeyQuotaWindowObservation>,
) -> Result<ProviderKeyQuotaObservation, DataLayerError> {
    Ok(ProviderKeyQuotaObservation {
        provider_api_key_id: row.try_get("provider_api_key_id").map_sql_err()?,
        provider_id: row.try_get("provider_id").map_sql_err()?,
        provider_api_key_name: row.try_get("provider_api_key_name").map_sql_err()?,
        provider_type: row.try_get("provider_type").map_sql_err()?,
        bucket_start_unix_secs: sqlite_u64(row, "bucket_start_unix_secs")?,
        observed_at_unix_secs: sqlite_u64(row, "observed_at_unix_secs")?,
        source: row.try_get("source").map_sql_err()?,
        plan_type: row.try_get("plan_type").map_sql_err()?,
        status_code: row.try_get("status_code").map_sql_err()?,
        status_label: row.try_get("status_label").map_sql_err()?,
        freshness: row.try_get("freshness").map_sql_err()?,
        credits_balance: sqlite_optional_real(row, "credits_balance")?,
        credits_unlimited: row.try_get("credits_unlimited").map_sql_err()?,
        reset_credits_count: sqlite_u64(row, "reset_credits_count")?,
        windows,
    })
}

fn map_window_row(row: &SqliteRow) -> Result<ProviderKeyQuotaWindowObservation, DataLayerError> {
    Ok(ProviderKeyQuotaWindowObservation {
        window_identity: row.try_get("window_identity").map_sql_err()?,
        code: row.try_get("code").map_sql_err()?,
        label: row.try_get("label").map_sql_err()?,
        scope: row.try_get("scope").map_sql_err()?,
        model: row.try_get("model").map_sql_err()?,
        unit: row.try_get("unit").map_sql_err()?,
        used_percent: sqlite_optional_real(row, "used_percent")?,
        remaining_percent: sqlite_optional_real(row, "remaining_percent")?,
        used_value: sqlite_optional_real(row, "used_value")?,
        remaining_value: sqlite_optional_real(row, "remaining_value")?,
        limit_value: sqlite_optional_real(row, "limit_value")?,
        reset_at_unix_secs: sqlite_optional_u64(row, "reset_at_unix_secs")?,
        window_minutes: sqlite_optional_u64(row, "window_minutes")?,
        exhausted: row.try_get("exhausted").map_sql_err()?,
        local_request_count: sqlite_u64(row, "local_request_count")?,
        local_total_tokens: sqlite_u64(row, "local_total_tokens")?,
        local_cost_usd: sqlite_real(row, "local_cost_usd")?,
    })
}

fn sqlite_u64(row: &SqliteRow, column: &str) -> Result<u64, DataLayerError> {
    let value: i64 = row.try_get(column).map_sql_err()?;
    u64::try_from(value)
        .map_err(|_| DataLayerError::UnexpectedValue(format!("{column} is negative")))
}

fn sqlite_optional_u64(row: &SqliteRow, column: &str) -> Result<Option<u64>, DataLayerError> {
    let value: Option<i64> = row.try_get(column).map_sql_err()?;
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

fn map_row(row: &SqliteRow) -> Result<StoredProviderQuotaSnapshot, DataLayerError> {
    StoredProviderQuotaSnapshot::new(
        row.try_get("provider_id").map_sql_err()?,
        row.try_get("billing_type").map_sql_err()?,
        sqlite_optional_real(row, "monthly_quota_usd")?,
        sqlite_real(row, "monthly_used_usd")?,
        row.try_get("quota_reset_day").map_sql_err()?,
        row.try_get("quota_last_reset_at_unix_secs").map_sql_err()?,
        row.try_get("quota_expires_at_unix_secs").map_sql_err()?,
        row.try_get("is_active").map_sql_err()?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqliteProviderQuotaRepository;
    use aether_data_contracts::repository::quota::{
        ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery,
        ProviderKeyQuotaWindowObservation, ProviderQuotaReadRepository,
        ProviderQuotaWriteRepository,
    };

    use crate::run_migrations;

    #[tokio::test]
    async fn sqlite_repository_reads_and_resets_provider_quotas() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool should connect");
        run_migrations(&pool)
            .await
            .expect("sqlite migrations should run");
        seed_provider_quotas(&pool).await;

        let repository = SqliteProviderQuotaRepository::new(pool);
        let quota = repository
            .find_by_provider_id("provider-1")
            .await
            .expect("quota should load")
            .expect("quota should exist");
        assert_eq!(quota.monthly_used_usd, 5.0);

        let quota = repository
            .find_by_provider_id("provider-null-used")
            .await
            .expect("quota with null usage should load")
            .expect("quota with null usage should exist");
        assert_eq!(quota.monthly_used_usd, 0.0);

        let quotas = repository
            .find_by_provider_ids(&["provider-2".to_string(), "provider-1".to_string()])
            .await
            .expect("quotas should load");
        assert_eq!(
            quotas
                .iter()
                .map(|quota| quota.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec!["provider-1", "provider-2"]
        );

        let reset = repository
            .reset_due(1_000 + 7 * 24 * 60 * 60)
            .await
            .expect("quota reset should run");
        assert_eq!(reset, 1);
        let quota = repository
            .find_by_provider_id("provider-1")
            .await
            .expect("quota should reload")
            .expect("quota should exist");
        assert_eq!(quota.monthly_used_usd, 0.0);
        assert_eq!(quota.quota_last_reset_at_unix_secs, Some(605_800));
    }

    #[tokio::test]
    async fn sqlite_key_quota_history_upserts_newest_observation_in_bucket() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("sqlite pool should connect");
        run_migrations(&pool)
            .await
            .expect("sqlite migrations should run");
        let repository = SqliteProviderQuotaRepository::new(pool);
        let observation = |observed_at, remaining| ProviderKeyQuotaObservation {
            provider_id: "provider-1".into(),
            provider_api_key_id: "key-1".into(),
            provider_api_key_name: "Key One".into(),
            provider_type: "codex".into(),
            bucket_start_unix_secs: 1_500,
            observed_at_unix_secs: observed_at,
            source: "test".into(),
            plan_type: Some("plus".into()),
            status_code: None,
            status_label: None,
            freshness: None,
            credits_balance: None,
            credits_unlimited: None,
            reset_credits_count: 0,
            windows: vec![ProviderKeyQuotaWindowObservation {
                window_identity: "weekly|||0".into(),
                code: "weekly".into(),
                label: "周额度".into(),
                scope: None,
                model: None,
                unit: Some("percent".into()),
                used_percent: Some(100.0 - remaining),
                remaining_percent: Some(remaining),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                reset_at_unix_secs: Some(10_000),
                window_minutes: Some(10_080),
                exhausted: false,
                local_request_count: 2,
                local_total_tokens: 100,
                local_cost_usd: 0.25,
            }],
        };

        assert!(repository
            .upsert_key_quota_observation(&observation(1_700, 80.0))
            .await
            .expect("first observation should write"));
        assert!(!repository
            .upsert_key_quota_observation(&observation(1_650, 90.0))
            .await
            .expect("old observation should be ignored"));
        assert!(repository
            .upsert_key_quota_observation(&observation(1_720, 75.0))
            .await
            .expect("new observation should replace bucket"));

        let stored = repository
            .list_key_quota_observations(&ProviderKeyQuotaObservationQuery {
                provider_id: "provider-1".into(),
                ..ProviderKeyQuotaObservationQuery::default()
            })
            .await
            .expect("history should load");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].observed_at_unix_secs, 1_720);
        assert_eq!(stored[0].windows[0].remaining_percent, Some(75.0));
    }

    async fn seed_provider_quotas(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
INSERT INTO providers (
  id, name, provider_type, billing_type, monthly_quota_usd, monthly_used_usd,
  quota_reset_day, quota_last_reset_at, is_active, created_at, updated_at
)
VALUES
  ('provider-1', 'Provider One', 'openai', 'monthly_quota', 20.0, 5.0, 7, 1000, 1, 1, 1),
  ('provider-2', 'Provider Two', 'openai', 'payg', NULL, 1.5, NULL, NULL, 1, 1, 1),
  ('provider-null-used', 'Provider Null Used', 'openai', 'payg', NULL, NULL, NULL, NULL, 1, 1, 1)
"#,
        )
        .execute(pool)
        .await
        .expect("providers should seed");
    }
}
