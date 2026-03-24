use async_trait::async_trait;
use sqlx::{postgres::PgRow, PgPool, Postgres, QueryBuilder, Row};

use super::types::{
    ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use crate::DataLayerError;

const LIST_PROVIDERS_BY_IDS_PREFIX: &str = r#"
SELECT
  id,
  name,
  website,
  provider_type
FROM providers
WHERE id IN (
"#;

const LIST_ENDPOINTS_BY_IDS_PREFIX: &str = r#"
SELECT
  id,
  provider_id,
  api_format,
  api_family,
  endpoint_kind,
  is_active
FROM provider_endpoints
WHERE id IN (
"#;

const LIST_KEYS_BY_IDS_PREFIX: &str = r#"
SELECT
  id,
  provider_id,
  name,
  auth_type,
  capabilities,
  is_active
FROM provider_api_keys
WHERE id IN (
"#;

#[derive(Debug, Clone)]
pub struct SqlxProviderCatalogReadRepository {
    pool: PgPool,
}

impl SqlxProviderCatalogReadRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn list_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, DataLayerError> {
        if provider_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = build_list_query(
            LIST_PROVIDERS_BY_IDS_PREFIX,
            provider_ids,
            " ORDER BY name ASC",
        )
        .build()
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_provider_row).collect()
    }

    pub async fn list_endpoints_by_ids(
        &self,
        endpoint_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogEndpoint>, DataLayerError> {
        if endpoint_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = build_list_query(
            LIST_ENDPOINTS_BY_IDS_PREFIX,
            endpoint_ids,
            " ORDER BY api_format ASC, id ASC",
        )
        .build()
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_endpoint_row).collect()
    }

    pub async fn list_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, DataLayerError> {
        if key_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = build_list_query(
            LIST_KEYS_BY_IDS_PREFIX,
            key_ids,
            " ORDER BY name ASC, id ASC",
        )
        .build()
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_key_row).collect()
    }
}

#[async_trait]
impl ProviderCatalogReadRepository for SqlxProviderCatalogReadRepository {
    async fn list_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, DataLayerError> {
        Self::list_providers_by_ids(self, provider_ids).await
    }

    async fn list_endpoints_by_ids(
        &self,
        endpoint_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogEndpoint>, DataLayerError> {
        Self::list_endpoints_by_ids(self, endpoint_ids).await
    }

    async fn list_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, DataLayerError> {
        Self::list_keys_by_ids(self, key_ids).await
    }
}

fn build_list_query<'a>(
    prefix: &'static str,
    ids: &'a [String],
    suffix: &'static str,
) -> QueryBuilder<'a, Postgres> {
    let mut builder = QueryBuilder::<Postgres>::new(prefix);
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    builder.push(suffix);
    builder
}

fn map_provider_row(row: &PgRow) -> Result<StoredProviderCatalogProvider, DataLayerError> {
    StoredProviderCatalogProvider::new(
        row.try_get("id")?,
        row.try_get("name")?,
        row.try_get("website")?,
        row.try_get("provider_type")?,
    )
}

fn map_endpoint_row(row: &PgRow) -> Result<StoredProviderCatalogEndpoint, DataLayerError> {
    StoredProviderCatalogEndpoint::new(
        row.try_get("id")?,
        row.try_get("provider_id")?,
        row.try_get("api_format")?,
        row.try_get("api_family")?,
        row.try_get("endpoint_kind")?,
        row.try_get("is_active")?,
    )
}

fn map_key_row(row: &PgRow) -> Result<StoredProviderCatalogKey, DataLayerError> {
    StoredProviderCatalogKey::new(
        row.try_get("id")?,
        row.try_get("provider_id")?,
        row.try_get("name")?,
        row.try_get("auth_type")?,
        row.try_get("capabilities")?,
        row.try_get("is_active")?,
    )
}

#[cfg(test)]
mod tests {
    use super::SqlxProviderCatalogReadRepository;
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
        let repository = SqlxProviderCatalogReadRepository::new(pool);
        let _ = repository.pool();
    }
}
