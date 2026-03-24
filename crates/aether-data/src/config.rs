use crate::postgres::PostgresPoolConfig;
use crate::redis::RedisClientConfig;
use crate::DataLayerError;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DataLayerConfig {
    pub postgres: Option<PostgresPoolConfig>,
    pub redis: Option<RedisClientConfig>,
}

impl DataLayerConfig {
    pub fn validate(&self) -> Result<(), DataLayerError> {
        if let Some(postgres) = &self.postgres {
            postgres.validate()?;
        }
        if let Some(redis) = &self.redis {
            redis.validate()?;
        }
        Ok(())
    }

    pub fn has_persistent_backends(&self) -> bool {
        self.postgres.is_some() || self.redis.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::DataLayerConfig;
    use crate::postgres::PostgresPoolConfig;
    use crate::redis::RedisClientConfig;

    #[test]
    fn validates_nested_backend_configs() {
        let config = DataLayerConfig {
            postgres: Some(PostgresPoolConfig {
                database_url: "postgres://localhost/aether".to_string(),
                min_connections: 2,
                max_connections: 8,
                acquire_timeout_ms: 1_500,
                idle_timeout_ms: 5_000,
                max_lifetime_ms: 30_000,
                statement_cache_capacity: 64,
                require_ssl: false,
            }),
            redis: Some(RedisClientConfig {
                url: "redis://127.0.0.1/0".to_string(),
                key_prefix: Some("aether".to_string()),
            }),
        };

        assert!(config.validate().is_ok());
        assert!(config.has_persistent_backends());
    }

    #[test]
    fn rejects_invalid_nested_backend_configs() {
        let config = DataLayerConfig {
            postgres: Some(PostgresPoolConfig {
                database_url: String::new(),
                min_connections: 4,
                max_connections: 2,
                acquire_timeout_ms: 1_500,
                idle_timeout_ms: 5_000,
                max_lifetime_ms: 30_000,
                statement_cache_capacity: 64,
                require_ssl: false,
            }),
            redis: None,
        };

        assert!(config.validate().is_err());
    }
}
