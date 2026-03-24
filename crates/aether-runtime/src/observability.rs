use crate::tracing::LogFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceObservabilityConfig {
    pub log_format: LogFormat,
    pub metrics_namespace: &'static str,
}

impl ServiceObservabilityConfig {
    pub const fn new(log_format: LogFormat, metrics_namespace: &'static str) -> Self {
        Self {
            log_format,
            metrics_namespace,
        }
    }
}
