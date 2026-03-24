use aether_executor::server::build_router_with_request_gates;
use aether_runtime::DistributedConcurrencyGate;

use crate::server::SpawnedServer;

#[derive(Debug, Clone, Default)]
pub struct ExecutorHarnessConfig {
    pub max_in_flight_requests: Option<usize>,
    pub distributed_request_gate: Option<DistributedConcurrencyGate>,
}

#[derive(Debug)]
pub struct ExecutorHarness {
    server: SpawnedServer,
}

impl ExecutorHarness {
    pub async fn start(config: ExecutorHarnessConfig) -> Result<Self, String> {
        Self::start_with_server(config, None).await
    }

    pub async fn start_on_port(config: ExecutorHarnessConfig, port: u16) -> Result<Self, String> {
        Self::start_with_server(config, Some(port)).await
    }

    async fn start_with_server(
        config: ExecutorHarnessConfig,
        port: Option<u16>,
    ) -> Result<Self, String> {
        let router = build_router_with_request_gates(
            config.max_in_flight_requests,
            config.distributed_request_gate,
        );
        let server = match port {
            Some(port) => SpawnedServer::start_on_port(port, router)
                .await
                .map_err(|err| format!("failed to start executor harness: {err}"))?,
            None => SpawnedServer::start(router)
                .await
                .map_err(|err| format!("failed to start executor harness: {err}"))?,
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
