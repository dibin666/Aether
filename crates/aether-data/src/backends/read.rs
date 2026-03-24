use std::fmt;
use std::sync::Arc;

use super::PostgresBackend;
use crate::repository::auth::AuthApiKeyReadRepository;
use crate::repository::candidates::RequestCandidateReadRepository;
use crate::repository::provider_catalog::ProviderCatalogReadRepository;
use crate::repository::shadow_results::ShadowResultReadRepository;
use crate::repository::usage::UsageReadRepository;
use crate::repository::video_tasks::VideoTaskReadRepository;

#[derive(Clone, Default)]
pub struct DataReadRepositories {
    auth_api_keys: Option<Arc<dyn AuthApiKeyReadRepository>>,
    request_candidates: Option<Arc<dyn RequestCandidateReadRepository>>,
    provider_catalog: Option<Arc<dyn ProviderCatalogReadRepository>>,
    usage: Option<Arc<dyn UsageReadRepository>>,
    video_tasks: Option<Arc<dyn VideoTaskReadRepository>>,
    shadow_results: Option<Arc<dyn ShadowResultReadRepository>>,
}

impl fmt::Debug for DataReadRepositories {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataReadRepositories")
            .field("has_auth_api_keys", &self.auth_api_keys.is_some())
            .field("has_request_candidates", &self.request_candidates.is_some())
            .field("has_provider_catalog", &self.provider_catalog.is_some())
            .field("has_usage", &self.usage.is_some())
            .field("has_video_tasks", &self.video_tasks.is_some())
            .field("has_shadow_results", &self.shadow_results.is_some())
            .finish()
    }
}

impl DataReadRepositories {
    pub(crate) fn from_postgres(postgres: Option<&PostgresBackend>) -> Self {
        Self {
            auth_api_keys: postgres.map(PostgresBackend::auth_api_key_read_repository),
            request_candidates: postgres.map(PostgresBackend::request_candidate_read_repository),
            provider_catalog: postgres.map(PostgresBackend::provider_catalog_read_repository),
            usage: postgres.map(PostgresBackend::usage_read_repository),
            video_tasks: postgres.map(PostgresBackend::video_task_read_repository),
            shadow_results: postgres.map(PostgresBackend::shadow_result_read_repository),
        }
    }

    pub fn auth_api_keys(&self) -> Option<Arc<dyn AuthApiKeyReadRepository>> {
        self.auth_api_keys.clone()
    }

    pub fn request_candidates(&self) -> Option<Arc<dyn RequestCandidateReadRepository>> {
        self.request_candidates.clone()
    }

    pub fn provider_catalog(&self) -> Option<Arc<dyn ProviderCatalogReadRepository>> {
        self.provider_catalog.clone()
    }

    pub fn usage(&self) -> Option<Arc<dyn UsageReadRepository>> {
        self.usage.clone()
    }

    pub fn video_tasks(&self) -> Option<Arc<dyn VideoTaskReadRepository>> {
        self.video_tasks.clone()
    }

    pub fn shadow_results(&self) -> Option<Arc<dyn ShadowResultReadRepository>> {
        self.shadow_results.clone()
    }

    pub fn has_any(&self) -> bool {
        self.auth_api_keys.is_some()
            || self.request_candidates.is_some()
            || self.provider_catalog.is_some()
            || self.usage.is_some()
            || self.video_tasks.is_some()
            || self.shadow_results.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::DataReadRepositories;
    use crate::backends::PostgresBackend;
    use crate::postgres::PostgresPoolConfig;

    #[tokio::test]
    async fn builds_read_repositories_from_postgres_backend() {
        let backend = PostgresBackend::from_config(PostgresPoolConfig {
            database_url: "postgres://localhost/aether".to_string(),
            min_connections: 1,
            max_connections: 4,
            acquire_timeout_ms: 1_000,
            idle_timeout_ms: 5_000,
            max_lifetime_ms: 30_000,
            statement_cache_capacity: 64,
            require_ssl: false,
        })
        .expect("postgres backend should build");

        let read = DataReadRepositories::from_postgres(Some(&backend));

        assert!(read.has_any());
        assert!(read.auth_api_keys().is_some());
        assert!(read.request_candidates().is_some());
        assert!(read.provider_catalog().is_some());
        assert!(read.usage().is_some());
        assert!(read.video_tasks().is_some());
        assert!(read.shadow_results().is_some());
    }
}
