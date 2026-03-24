use aether_gateway::{build_router_with_state, AppState};
use aether_runtime::DistributedConcurrencyGate;

use crate::server::SpawnedServer;

#[derive(Debug, Clone)]
pub struct GatewayHarnessConfig {
    pub upstream_base_url: String,
    pub control_base_url: Option<String>,
    pub executor_base_url: Option<String>,
    pub max_in_flight_requests: Option<usize>,
    pub distributed_request_gate: Option<DistributedConcurrencyGate>,
}

impl GatewayHarnessConfig {
    pub fn new(upstream_base_url: impl Into<String>) -> Self {
        Self {
            upstream_base_url: upstream_base_url.into(),
            control_base_url: None,
            executor_base_url: None,
            max_in_flight_requests: None,
            distributed_request_gate: None,
        }
    }
}

#[derive(Debug)]
pub struct GatewayHarness {
    server: SpawnedServer,
}

impl GatewayHarness {
    pub async fn start(config: GatewayHarnessConfig) -> Result<Self, String> {
        Self::start_with_server(config, None).await
    }

    pub async fn start_on_port(config: GatewayHarnessConfig, port: u16) -> Result<Self, String> {
        Self::start_with_server(config, Some(port)).await
    }

    async fn start_with_server(
        config: GatewayHarnessConfig,
        port: Option<u16>,
    ) -> Result<Self, String> {
        let mut state = AppState::new_with_executor(
            config.upstream_base_url,
            config.control_base_url,
            config.executor_base_url,
        )
        .map_err(|err| format!("failed to build gateway harness state: {err}"))?;
        if let Some(limit) = config.max_in_flight_requests {
            state = state.with_request_concurrency_limit(limit);
        }
        if let Some(gate) = config.distributed_request_gate {
            state = state.with_distributed_request_concurrency_gate(gate);
        }
        let router = build_router_with_state(state);
        let server = match port {
            Some(port) => SpawnedServer::start_on_port(port, router)
                .await
                .map_err(|err| format!("failed to start gateway harness: {err}"))?,
            None => SpawnedServer::start(router)
                .await
                .map_err(|err| format!("failed to start gateway harness: {err}"))?,
        };
        Ok(Self { server })
    }

    pub fn base_url(&self) -> &str {
        self.server.base_url()
    }

    pub fn port(&self) -> u16 {
        self.server.port()
    }
}
