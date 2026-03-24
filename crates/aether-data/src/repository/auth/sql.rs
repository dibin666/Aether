use async_trait::async_trait;
use sqlx::{PgPool, Row};

use super::types::{AuthApiKeyLookupKey, AuthApiKeyReadRepository, StoredAuthApiKeySnapshot};
use crate::DataLayerError;

const FIND_BY_KEY_HASH_SQL: &str = r#"
SELECT
  users.id AS user_id,
  users.username,
  users.email,
  users.role::text AS user_role,
  users.auth_source::text AS user_auth_source,
  users.is_active AS user_is_active,
  users.is_deleted AS user_is_deleted,
  users.allowed_providers AS user_allowed_providers,
  users.allowed_api_formats AS user_allowed_api_formats,
  users.allowed_models AS user_allowed_models,
  api_keys.id AS api_key_id,
  api_keys.name AS api_key_name,
  api_keys.is_active AS api_key_is_active,
  api_keys.is_locked AS api_key_is_locked,
  api_keys.is_standalone AS api_key_is_standalone,
  api_keys.rate_limit AS api_key_rate_limit,
  api_keys.concurrent_limit AS api_key_concurrent_limit,
  CAST(EXTRACT(EPOCH FROM api_keys.expires_at) AS BIGINT) AS api_key_expires_at_unix_secs,
  api_keys.allowed_providers AS api_key_allowed_providers,
  api_keys.allowed_api_formats AS api_key_allowed_api_formats,
  api_keys.allowed_models AS api_key_allowed_models
FROM api_keys
JOIN users ON users.id = api_keys.user_id
WHERE api_keys.key_hash = $1
LIMIT 1
"#;

const FIND_BY_API_KEY_ID_SQL: &str = r#"
SELECT
  users.id AS user_id,
  users.username,
  users.email,
  users.role::text AS user_role,
  users.auth_source::text AS user_auth_source,
  users.is_active AS user_is_active,
  users.is_deleted AS user_is_deleted,
  users.allowed_providers AS user_allowed_providers,
  users.allowed_api_formats AS user_allowed_api_formats,
  users.allowed_models AS user_allowed_models,
  api_keys.id AS api_key_id,
  api_keys.name AS api_key_name,
  api_keys.is_active AS api_key_is_active,
  api_keys.is_locked AS api_key_is_locked,
  api_keys.is_standalone AS api_key_is_standalone,
  api_keys.rate_limit AS api_key_rate_limit,
  api_keys.concurrent_limit AS api_key_concurrent_limit,
  CAST(EXTRACT(EPOCH FROM api_keys.expires_at) AS BIGINT) AS api_key_expires_at_unix_secs,
  api_keys.allowed_providers AS api_key_allowed_providers,
  api_keys.allowed_api_formats AS api_key_allowed_api_formats,
  api_keys.allowed_models AS api_key_allowed_models
FROM api_keys
JOIN users ON users.id = api_keys.user_id
WHERE api_keys.id = $1
LIMIT 1
"#;

const FIND_BY_USER_API_KEY_IDS_SQL: &str = r#"
SELECT
  users.id AS user_id,
  users.username,
  users.email,
  users.role::text AS user_role,
  users.auth_source::text AS user_auth_source,
  users.is_active AS user_is_active,
  users.is_deleted AS user_is_deleted,
  users.allowed_providers AS user_allowed_providers,
  users.allowed_api_formats AS user_allowed_api_formats,
  users.allowed_models AS user_allowed_models,
  api_keys.id AS api_key_id,
  api_keys.name AS api_key_name,
  api_keys.is_active AS api_key_is_active,
  api_keys.is_locked AS api_key_is_locked,
  api_keys.is_standalone AS api_key_is_standalone,
  api_keys.rate_limit AS api_key_rate_limit,
  api_keys.concurrent_limit AS api_key_concurrent_limit,
  CAST(EXTRACT(EPOCH FROM api_keys.expires_at) AS BIGINT) AS api_key_expires_at_unix_secs,
  api_keys.allowed_providers AS api_key_allowed_providers,
  api_keys.allowed_api_formats AS api_key_allowed_api_formats,
  api_keys.allowed_models AS api_key_allowed_models
FROM api_keys
JOIN users ON users.id = api_keys.user_id
WHERE api_keys.id = $1 AND users.id = $2
LIMIT 1
"#;

#[derive(Debug, Clone)]
pub struct SqlxAuthApiKeySnapshotReadRepository {
    pool: PgPool,
}

impl SqlxAuthApiKeySnapshotReadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn find_api_key_snapshot(
        &self,
        key: AuthApiKeyLookupKey<'_>,
    ) -> Result<Option<StoredAuthApiKeySnapshot>, DataLayerError> {
        let row = match key {
            AuthApiKeyLookupKey::KeyHash(key_hash) => {
                sqlx::query(FIND_BY_KEY_HASH_SQL)
                    .bind(key_hash)
                    .fetch_optional(&self.pool)
                    .await?
            }
            AuthApiKeyLookupKey::ApiKeyId(api_key_id) => {
                sqlx::query(FIND_BY_API_KEY_ID_SQL)
                    .bind(api_key_id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            AuthApiKeyLookupKey::UserApiKeyIds {
                user_id,
                api_key_id,
            } => {
                sqlx::query(FIND_BY_USER_API_KEY_IDS_SQL)
                    .bind(api_key_id)
                    .bind(user_id)
                    .fetch_optional(&self.pool)
                    .await?
            }
        };

        row.as_ref().map(map_auth_api_key_snapshot_row).transpose()
    }
}

#[async_trait]
impl AuthApiKeyReadRepository for SqlxAuthApiKeySnapshotReadRepository {
    async fn find_api_key_snapshot(
        &self,
        key: AuthApiKeyLookupKey<'_>,
    ) -> Result<Option<StoredAuthApiKeySnapshot>, DataLayerError> {
        Self::find_api_key_snapshot(self, key).await
    }
}

fn map_auth_api_key_snapshot_row(
    row: &sqlx::postgres::PgRow,
) -> Result<StoredAuthApiKeySnapshot, DataLayerError> {
    StoredAuthApiKeySnapshot::new(
        row.try_get("user_id")?,
        row.try_get("username")?,
        row.try_get("email")?,
        row.try_get("user_role")?,
        row.try_get("user_auth_source")?,
        row.try_get("user_is_active")?,
        row.try_get("user_is_deleted")?,
        row.try_get("user_allowed_providers")?,
        row.try_get("user_allowed_api_formats")?,
        row.try_get("user_allowed_models")?,
        row.try_get("api_key_id")?,
        row.try_get("api_key_name")?,
        row.try_get("api_key_is_active")?,
        row.try_get("api_key_is_locked")?,
        row.try_get("api_key_is_standalone")?,
        row.try_get("api_key_rate_limit")?,
        row.try_get("api_key_concurrent_limit")?,
        row.try_get("api_key_expires_at_unix_secs")?,
        row.try_get("api_key_allowed_providers")?,
        row.try_get("api_key_allowed_api_formats")?,
        row.try_get("api_key_allowed_models")?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxAuthApiKeySnapshotReadRepository;
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
        let repository = SqlxAuthApiKeySnapshotReadRepository::new(pool);
        let _ = repository.pool();
    }
}
