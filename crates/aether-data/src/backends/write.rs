use std::fmt;
use std::sync::Arc;

use super::PostgresBackend;
use crate::repository::shadow_results::ShadowResultWriteRepository;

#[derive(Clone, Default)]
pub struct DataWriteRepositories {
    shadow_results: Option<Arc<dyn ShadowResultWriteRepository>>,
}

impl fmt::Debug for DataWriteRepositories {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataWriteRepositories")
            .field("has_shadow_results", &self.shadow_results.is_some())
            .finish()
    }
}

impl DataWriteRepositories {
    pub(crate) fn from_postgres(postgres: Option<&PostgresBackend>) -> Self {
        Self {
            shadow_results: postgres.map(PostgresBackend::shadow_result_write_repository),
        }
    }

    pub fn shadow_results(&self) -> Option<Arc<dyn ShadowResultWriteRepository>> {
        self.shadow_results.clone()
    }

    pub fn has_any(&self) -> bool {
        self.shadow_results.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::DataWriteRepositories;
    use crate::backends::PostgresBackend;
    use crate::postgres::PostgresPoolConfig;

    #[tokio::test]
    async fn builds_shadow_result_writer_from_postgres_backend() {
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

        let write = DataWriteRepositories::from_postgres(Some(&backend));

        assert!(write.has_any());
        assert!(write.shadow_results().is_some());
    }
}
