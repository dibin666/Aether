use std::sync::Arc;

use crate::postgres::{
    PostgresLeaseRunner, PostgresLeaseRunnerConfig, PostgresPool, PostgresPoolConfig,
    PostgresPoolFactory, PostgresTransactionRunner,
};
use crate::repository::auth::{AuthApiKeyReadRepository, SqlxAuthApiKeySnapshotReadRepository};
use crate::repository::candidates::{
    RequestCandidateReadRepository, SqlxRequestCandidateReadRepository,
};
use crate::repository::provider_catalog::{
    ProviderCatalogReadRepository, SqlxProviderCatalogReadRepository,
};
use crate::repository::shadow_results::{
    ShadowResultReadRepository, ShadowResultWriteRepository, SqlxShadowResultRepository,
};
use crate::repository::usage::{SqlxUsageReadRepository, UsageReadRepository};
use crate::repository::video_tasks::{SqlxVideoTaskReadRepository, VideoTaskReadRepository};
use crate::DataLayerError;

#[derive(Debug, Clone)]
pub struct PostgresBackend {
    config: PostgresPoolConfig,
    pool: PostgresPool,
}

impl PostgresBackend {
    pub fn from_config(config: PostgresPoolConfig) -> Result<Self, DataLayerError> {
        let factory = PostgresPoolFactory::new(config.clone())?;
        let pool = factory.connect_lazy()?;

        Ok(Self { config, pool })
    }

    pub fn config(&self) -> &PostgresPoolConfig {
        &self.config
    }

    pub fn pool(&self) -> &PostgresPool {
        &self.pool
    }

    pub fn pool_clone(&self) -> PostgresPool {
        self.pool.clone()
    }

    pub fn auth_api_key_read_repository(&self) -> Arc<dyn AuthApiKeyReadRepository> {
        Arc::new(SqlxAuthApiKeySnapshotReadRepository::new(self.pool_clone()))
    }

    pub fn request_candidate_read_repository(&self) -> Arc<dyn RequestCandidateReadRepository> {
        Arc::new(SqlxRequestCandidateReadRepository::new(self.pool_clone()))
    }

    pub fn provider_catalog_read_repository(&self) -> Arc<dyn ProviderCatalogReadRepository> {
        Arc::new(SqlxProviderCatalogReadRepository::new(self.pool_clone()))
    }

    pub fn usage_read_repository(&self) -> Arc<dyn UsageReadRepository> {
        Arc::new(SqlxUsageReadRepository::new(self.pool_clone()))
    }

    pub fn video_task_read_repository(&self) -> Arc<dyn VideoTaskReadRepository> {
        Arc::new(SqlxVideoTaskReadRepository::new(self.pool_clone()))
    }

    pub fn transaction_runner(&self) -> PostgresTransactionRunner {
        PostgresTransactionRunner::new(self.pool_clone())
    }

    pub fn lease_runner(
        &self,
        config: PostgresLeaseRunnerConfig,
    ) -> Result<PostgresLeaseRunner, DataLayerError> {
        PostgresLeaseRunner::new(self.transaction_runner(), config)
    }

    pub fn shadow_result_write_repository(&self) -> Arc<dyn ShadowResultWriteRepository> {
        Arc::new(SqlxShadowResultRepository::new(self.pool_clone()))
    }

    pub fn shadow_result_read_repository(&self) -> Arc<dyn ShadowResultReadRepository> {
        Arc::new(SqlxShadowResultRepository::new(self.pool_clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresBackend;
    use crate::postgres::{PostgresLeaseRunnerConfig, PostgresPoolConfig};

    #[tokio::test]
    async fn backend_retains_config_and_pool() {
        let config = PostgresPoolConfig {
            database_url: "postgres://localhost/aether".to_string(),
            min_connections: 1,
            max_connections: 4,
            acquire_timeout_ms: 1_000,
            idle_timeout_ms: 5_000,
            max_lifetime_ms: 30_000,
            statement_cache_capacity: 64,
            require_ssl: false,
        };

        let backend =
            PostgresBackend::from_config(config.clone()).expect("backend should build lazily");

        assert_eq!(backend.config(), &config);
        let _pool = backend.pool();
        let _pool_clone = backend.pool_clone();
        let _auth_api_key_reader = backend.auth_api_key_read_repository();
        let _request_candidate_reader = backend.request_candidate_read_repository();
        let _provider_catalog_reader = backend.provider_catalog_read_repository();
        let _usage_reader = backend.usage_read_repository();
        let _video_task_reader = backend.video_task_read_repository();
        let _transaction_runner = backend.transaction_runner();
        let _lease_runner = backend
            .lease_runner(PostgresLeaseRunnerConfig::default())
            .expect("lease runner should build");
        let _shadow_result_reader = backend.shadow_result_read_repository();
        let _shadow_result_writer = backend.shadow_result_write_repository();
    }
}
