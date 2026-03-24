use crate::observability::ServiceObservabilityConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceRuntimeConfig {
    pub service_name: &'static str,
    pub default_log_filter: &'static str,
    pub observability: ServiceObservabilityConfig,
}

impl ServiceRuntimeConfig {
    pub const fn new(service_name: &'static str, default_log_filter: &'static str) -> Self {
        Self {
            service_name,
            default_log_filter,
            observability: ServiceObservabilityConfig::new(crate::LogFormat::Pretty, service_name),
        }
    }

    pub const fn with_log_format(mut self, log_format: crate::LogFormat) -> Self {
        self.observability.log_format = log_format;
        self
    }

    pub const fn with_metrics_namespace(mut self, metrics_namespace: &'static str) -> Self {
        self.observability.metrics_namespace = metrics_namespace;
        self
    }
}
