use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use aether_executor::server;
use aether_runtime::{
    init_service_runtime, DistributedConcurrencyGate, RedisDistributedConcurrencyConfig,
    ServiceRuntimeConfig,
};

#[derive(Parser, Debug)]
#[command(name = "aether-executor", about = "Internal Rust executor for Aether")]
struct Args {
    #[arg(long, env = "AETHER_EXECUTOR_TRANSPORT", default_value = "unix_socket")]
    transport: String,

    #[arg(long, env = "AETHER_EXECUTOR_BIND", default_value = "127.0.0.1:5219")]
    bind: String,

    #[arg(
        long,
        env = "AETHER_EXECUTOR_UNIX_SOCKET",
        default_value = "/tmp/aether-executor.sock"
    )]
    unix_socket: PathBuf,

    #[arg(long, env = "AETHER_EXECUTOR_MAX_IN_FLIGHT_REQUESTS")]
    max_in_flight_requests: Option<usize>,

    #[arg(long, env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_LIMIT")]
    distributed_request_limit: Option<usize>,

    #[arg(long, env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_REDIS_URL")]
    distributed_request_redis_url: Option<String>,

    #[arg(long, env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_REDIS_KEY_PREFIX")]
    distributed_request_redis_key_prefix: Option<String>,

    #[arg(
        long,
        env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_LEASE_TTL_MS",
        default_value_t = 30_000
    )]
    distributed_request_lease_ttl_ms: u64,

    #[arg(
        long,
        env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_RENEW_INTERVAL_MS",
        default_value_t = 10_000
    )]
    distributed_request_renew_interval_ms: u64,

    #[arg(
        long,
        env = "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_COMMAND_TIMEOUT_MS",
        default_value_t = 1_000
    )]
    distributed_request_command_timeout_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    init_service_runtime(ServiceRuntimeConfig::new(
        "aether-executor",
        "aether_executor=info",
    ))?;

    let args = Args::parse();
    let distributed_request_gate = match args.distributed_request_limit.filter(|limit| *limit > 0) {
        Some(limit) => {
            let redis_url = args
                .distributed_request_redis_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "AETHER_EXECUTOR_DISTRIBUTED_REQUEST_REDIS_URL is required when distributed request limit is enabled",
                    )
                })?;
            Some(DistributedConcurrencyGate::new_redis(
                "executor_requests_distributed",
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
            )?)
        }
        None => None,
    };
    match args.transport.trim().to_ascii_lowercase().as_str() {
        "unix_socket" | "unix" | "uds" => {
            info!(socket = %args.unix_socket.display(), "aether-executor started");
            server::serve_unix(
                &args.unix_socket,
                args.max_in_flight_requests,
                distributed_request_gate.clone(),
            )
            .await?;
        }
        "tcp" => {
            info!(
                bind = %args.bind,
                max_in_flight_requests = args.max_in_flight_requests.unwrap_or_default(),
                distributed_request_limit = args.distributed_request_limit.unwrap_or_default(),
                "aether-executor started"
            );
            server::serve_tcp(
                &args.bind,
                args.max_in_flight_requests,
                distributed_request_gate,
            )
            .await?;
        }
        other => {
            return Err(format!("unsupported executor transport: {other}").into());
        }
    }

    Ok(())
}
