use aether_data::postgres::PostgresPoolConfig;
use aether_data::DataLayerConfig;

#[derive(Debug, Clone, Default)]
pub struct GatewayDataConfig {
    postgres: Option<PostgresPoolConfig>,
}

impl GatewayDataConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn from_postgres_config(postgres: PostgresPoolConfig) -> Self {
        Self {
            postgres: Some(postgres),
        }
    }

    pub fn from_postgres_url(database_url: impl Into<String>, require_ssl: bool) -> Self {
        let mut postgres = PostgresPoolConfig::default();
        postgres.database_url = database_url.into();
        postgres.require_ssl = require_ssl;
        Self::from_postgres_config(postgres)
    }

    pub fn postgres(&self) -> Option<&PostgresPoolConfig> {
        self.postgres.as_ref()
    }

    pub fn is_enabled(&self) -> bool {
        self.postgres.is_some()
    }

    pub fn to_data_layer_config(&self) -> DataLayerConfig {
        DataLayerConfig {
            postgres: self.postgres.clone(),
            redis: None,
        }
    }
}
