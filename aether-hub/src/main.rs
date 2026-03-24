use std::net::SocketAddr;
use std::time::Duration;

use aether_hub::{build_router_with_state, AppState, ConnConfig, ControlPlaneClient};
use aether_runtime::{
    init_service_runtime, DistributedConcurrencyGate, RedisDistributedConcurrencyConfig,
    ServiceRuntimeConfig,
};
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "aether-hub", about = "Tunnel Hub for Aether")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8085", env = "TUNNEL_HUB_BIND")]
    bind: String,

    #[arg(long, default_value_t = 0, env = "TUNNEL_HUB_PROXY_IDLE_TIMEOUT")]
    proxy_idle_timeout: u64,

    #[arg(long, default_value_t = 15, env = "TUNNEL_HUB_PING_INTERVAL")]
    ping_interval: u64,

    #[arg(long, default_value_t = 2048, env = "TUNNEL_HUB_MAX_STREAMS")]
    max_streams: usize,

    #[arg(
        long,
        default_value_t = 128,
        env = "TUNNEL_HUB_OUTBOUND_QUEUE_CAPACITY"
    )]
    outbound_queue_capacity: usize,

    #[arg(
        long,
        default_value = "http://127.0.0.1:8084",
        env = "TUNNEL_HUB_APP_BASE_URL"
    )]
    app_base_url: String,

    #[arg(long, env = "TUNNEL_HUB_MAX_IN_FLIGHT_REQUESTS")]
    max_in_flight_requests: Option<usize>,

    #[arg(long, env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_LIMIT")]
    distributed_request_limit: Option<usize>,

    #[arg(long, env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_REDIS_URL")]
    distributed_request_redis_url: Option<String>,

    #[arg(long, env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_REDIS_KEY_PREFIX")]
    distributed_request_redis_key_prefix: Option<String>,

    #[arg(
        long,
        env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_LEASE_TTL_MS",
        default_value_t = 30_000
    )]
    distributed_request_lease_ttl_ms: u64,

    #[arg(
        long,
        env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_RENEW_INTERVAL_MS",
        default_value_t = 10_000
    )]
    distributed_request_renew_interval_ms: u64,

    #[arg(
        long,
        env = "TUNNEL_HUB_DISTRIBUTED_REQUEST_COMMAND_TIMEOUT_MS",
        default_value_t = 1_000
    )]
    distributed_request_command_timeout_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_service_runtime(ServiceRuntimeConfig::new("aether-hub", "aether_hub=info"))?;

    let args = Args::parse();
    let outbound_queue_capacity = args.outbound_queue_capacity.clamp(8, 4096);
    let ping_interval = Duration::from_secs(args.ping_interval);
    let mut state = AppState::new(
        ControlPlaneClient::new(args.app_base_url),
        ConnConfig {
            ping_interval,
            idle_timeout: Duration::from_secs(args.proxy_idle_timeout),
            outbound_queue_capacity,
        },
        args.max_streams,
    )
    .with_request_concurrency_limit(args.max_in_flight_requests);

    if let Some(limit) = args.distributed_request_limit.filter(|limit| *limit > 0) {
        let redis_url = args
            .distributed_request_redis_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "TUNNEL_HUB_DISTRIBUTED_REQUEST_REDIS_URL is required when distributed request limit is enabled",
                )
            })?;
        state = state.with_distributed_request_gate(DistributedConcurrencyGate::new_redis(
            "hub_requests_distributed",
            limit,
            RedisDistributedConcurrencyConfig {
                url: redis_url.to_string(),
                key_prefix: args
                    .distributed_request_redis_key_prefix
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                lease_ttl_ms: args.distributed_request_lease_ttl_ms.max(1),
                renew_interval_ms: args.distributed_request_renew_interval_ms.max(1),
                command_timeout_ms: Some(args.distributed_request_command_timeout_ms.max(1)),
            },
        )?);
    }

    let app = build_router_with_state(state);
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    info!(bind = %args.bind, "aether-hub started");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, StatusCode};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    async fn start_server(app: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should resolve");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn hub_exposes_metrics_endpoint() {
        let app = build_router_with_state(AppState::new(
            ControlPlaneClient::disabled(),
            ConnConfig {
                ping_interval: Duration::from_secs(15),
                idle_timeout: Duration::from_secs(0),
                outbound_queue_capacity: 128,
            },
            128,
        ));
        let (base_url, handle) = start_server(app).await;

        let response = reqwest::Client::new()
            .get(format!("{base_url}/metrics"))
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/plain; version=0.0.4; charset=utf-8")
        );
        let body = response.text().await.expect("body should read");
        assert!(body.contains("service_up{service=\"aether-hub\"} 1"));
        assert!(body.contains("hub_proxy_connections 0"));
        assert!(body.contains("hub_active_streams 0"));

        handle.abort();
    }

    #[tokio::test]
    async fn hub_rejects_second_proxy_connection_with_distributed_overload() {
        let distributed_gate =
            DistributedConcurrencyGate::new_in_memory("hub_requests_distributed", 1);
        let app_a = build_router_with_state(
            AppState::new(
                ControlPlaneClient::disabled(),
                ConnConfig {
                    ping_interval: Duration::from_secs(15),
                    idle_timeout: Duration::from_secs(0),
                    outbound_queue_capacity: 128,
                },
                128,
            )
            .with_distributed_request_gate(distributed_gate.clone()),
        );
        let app_b = build_router_with_state(
            AppState::new(
                ControlPlaneClient::disabled(),
                ConnConfig {
                    ping_interval: Duration::from_secs(15),
                    idle_timeout: Duration::from_secs(0),
                    outbound_queue_capacity: 128,
                },
                128,
            )
            .with_distributed_request_gate(distributed_gate),
        );
        let (base_a, handle_a) = start_server(app_a).await;
        let (base_b, handle_b) = start_server(app_b).await;

        let request_a = format!("{}/proxy", base_a.replace("http://", "ws://"))
            .into_client_request()
            .expect("request should build");
        let mut request_a = request_a;
        request_a
            .headers_mut()
            .insert("x-node-id", HeaderValue::from_static("node-a"));
        let (socket, _) = tokio_tungstenite::connect_async(request_a)
            .await
            .expect("first websocket should connect");

        let request_b = format!("{}/proxy", base_b.replace("http://", "ws://"))
            .into_client_request()
            .expect("request should build");
        let mut request_b = request_b;
        request_b
            .headers_mut()
            .insert("x-node-id", HeaderValue::from_static("node-b"));
        let error = tokio_tungstenite::connect_async(request_b)
            .await
            .expect_err("second websocket should be rejected");
        match error {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("unexpected websocket error: {other}"),
        }

        drop(socket);
        handle_a.abort();
        handle_b.abort();
    }
}
