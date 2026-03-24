use std::sync::OnceLock;

use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::config::ServiceRuntimeConfig;
use crate::error::RuntimeBootstrapError;

static TRACING_INIT: OnceLock<Result<(), String>> = OnceLock::new();

pub type LogReloader = Box<dyn Fn(&str) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

pub(crate) fn init_tracing(config: ServiceRuntimeConfig) -> Result<(), RuntimeBootstrapError> {
    TRACING_INIT
        .get_or_init(|| {
            let filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.default_log_filter.into());
            match config.observability.log_format {
                LogFormat::Pretty => tracing_subscriber::fmt().with_env_filter(filter).try_init(),
                LogFormat::Json => tracing_subscriber::fmt()
                    .json()
                    .with_env_filter(filter)
                    .try_init(),
            }
            .map_err(|err| err.to_string())
        })
        .clone()
        .map_err(RuntimeBootstrapError::Tracing)
}

pub fn init_reloadable_tracing(
    initial_filter: &str,
    format: LogFormat,
) -> Result<LogReloader, RuntimeBootstrapError> {
    use tracing_subscriber::reload;

    let filter = EnvFilter::try_new(initial_filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let (filter_layer, reload_handle) = reload::Layer::new(filter);

    match format {
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(filter_layer)
            .with(tracing_subscriber::fmt::layer())
            .try_init(),
        LogFormat::Json => tracing_subscriber::registry()
            .with(filter_layer)
            .with(tracing_subscriber::fmt::layer().json())
            .try_init(),
    }
    .map_err(|err| RuntimeBootstrapError::Tracing(err.to_string()))?;

    Ok(Box::new(move |level: &str| {
        if let Ok(new_filter) = EnvFilter::try_new(level) {
            let _ = reload_handle.modify(|filter| *filter = new_filter);
        }
    }))
}
