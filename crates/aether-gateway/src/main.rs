use clap::{Args as ClapArgs, Parser, ValueEnum};
use tracing::info;

use aether_data::postgres::PostgresPoolConfig;
use aether_gateway::{
    build_router_with_state, AppState, GatewayDataConfig, VideoTaskTruthSourceMode,
};
use aether_runtime::{
    init_service_runtime, DistributedConcurrencyGate, RedisDistributedConcurrencyConfig,
    ServiceRuntimeConfig,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum VideoTaskTruthSourceArg {
    PythonSyncReport,
    RustAuthoritative,
}

impl From<VideoTaskTruthSourceArg> for VideoTaskTruthSourceMode {
    fn from(value: VideoTaskTruthSourceArg) -> Self {
        match value {
            VideoTaskTruthSourceArg::PythonSyncReport => VideoTaskTruthSourceMode::PythonSyncReport,
            VideoTaskTruthSourceArg::RustAuthoritative => {
                VideoTaskTruthSourceMode::RustAuthoritative
            }
        }
    }
}

#[derive(ClapArgs, Debug, Clone)]
struct GatewayDataArgs {
    #[arg(long, env = "AETHER_GATEWAY_DATA_POSTGRES_URL")]
    postgres_url: Option<String>,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_MIN_CONNECTIONS",
        default_value_t = 1
    )]
    postgres_min_connections: u32,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_MAX_CONNECTIONS",
        default_value_t = 20
    )]
    postgres_max_connections: u32,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_ACQUIRE_TIMEOUT_MS",
        default_value_t = 5_000
    )]
    postgres_acquire_timeout_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_IDLE_TIMEOUT_MS",
        default_value_t = 60_000
    )]
    postgres_idle_timeout_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_MAX_LIFETIME_MS",
        default_value_t = 1_800_000
    )]
    postgres_max_lifetime_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_STATEMENT_CACHE_CAPACITY",
        default_value_t = 100
    )]
    postgres_statement_cache_capacity: usize,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DATA_POSTGRES_REQUIRE_SSL",
        default_value_t = false
    )]
    postgres_require_ssl: bool,
}

impl GatewayDataArgs {
    fn to_config(&self) -> GatewayDataConfig {
        let Some(database_url) = self
            .postgres_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return GatewayDataConfig::disabled();
        };

        GatewayDataConfig::from_postgres_config(PostgresPoolConfig {
            database_url: database_url.to_string(),
            min_connections: self.postgres_min_connections,
            max_connections: self.postgres_max_connections,
            acquire_timeout_ms: self.postgres_acquire_timeout_ms,
            idle_timeout_ms: self.postgres_idle_timeout_ms,
            max_lifetime_ms: self.postgres_max_lifetime_ms,
            statement_cache_capacity: self.postgres_statement_cache_capacity,
            require_ssl: self.postgres_require_ssl,
        })
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "aether-gateway",
    about = "Phase 3a Rust ingress gateway for Aether"
)]
struct Args {
    #[arg(long, env = "AETHER_GATEWAY_BIND", default_value = "0.0.0.0:8084")]
    bind: String,

    #[arg(
        long,
        env = "AETHER_GATEWAY_UPSTREAM",
        default_value = "http://127.0.0.1:18084"
    )]
    upstream: String,

    #[arg(long, env = "AETHER_GATEWAY_CONTROL_URL")]
    control_url: Option<String>,

    #[arg(long, env = "AETHER_GATEWAY_EXECUTOR_URL")]
    executor_url: Option<String>,

    #[arg(
        long,
        env = "AETHER_GATEWAY_VIDEO_TASK_TRUTH_SOURCE_MODE",
        value_enum,
        default_value = "python-sync-report"
    )]
    video_task_truth_source_mode: VideoTaskTruthSourceArg,

    #[arg(
        long,
        env = "AETHER_GATEWAY_VIDEO_TASK_POLLER_INTERVAL_MS",
        default_value_t = 5000
    )]
    video_task_poller_interval_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_VIDEO_TASK_POLLER_BATCH_SIZE",
        default_value_t = 32
    )]
    video_task_poller_batch_size: usize,

    #[arg(long, env = "AETHER_GATEWAY_VIDEO_TASK_STORE_PATH")]
    video_task_store_path: Option<String>,

    #[arg(long, env = "AETHER_GATEWAY_MAX_IN_FLIGHT_REQUESTS")]
    max_in_flight_requests: Option<usize>,

    #[arg(long, env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_LIMIT")]
    distributed_request_limit: Option<usize>,

    #[arg(long, env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_REDIS_URL")]
    distributed_request_redis_url: Option<String>,

    #[arg(long, env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_REDIS_KEY_PREFIX")]
    distributed_request_redis_key_prefix: Option<String>,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_LEASE_TTL_MS",
        default_value_t = 30_000
    )]
    distributed_request_lease_ttl_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_RENEW_INTERVAL_MS",
        default_value_t = 10_000
    )]
    distributed_request_renew_interval_ms: u64,

    #[arg(
        long,
        env = "AETHER_GATEWAY_DISTRIBUTED_REQUEST_COMMAND_TIMEOUT_MS",
        default_value_t = 1_000
    )]
    distributed_request_command_timeout_ms: u64,

    #[command(flatten)]
    data: GatewayDataArgs,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_service_runtime(ServiceRuntimeConfig::new(
        "aether-gateway",
        "aether_gateway=info",
    ))?;

    let args = Args::parse();
    let control_url = args
        .control_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let executor_url = args
        .executor_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    info!(
        bind = %args.bind,
        upstream = %args.upstream,
        control_url = control_url.unwrap_or("-"),
        executor_url = executor_url.unwrap_or("-"),
        video_task_truth_source_mode = ?args.video_task_truth_source_mode,
        video_task_poller_interval_ms = args.video_task_poller_interval_ms,
        video_task_poller_batch_size = args.video_task_poller_batch_size,
        video_task_store_path = args.video_task_store_path.as_deref().unwrap_or("-"),
        max_in_flight_requests = args.max_in_flight_requests.unwrap_or_default(),
        distributed_request_limit = args.distributed_request_limit.unwrap_or_default(),
        distributed_request_redis_url = args
            .distributed_request_redis_url
            .as_deref()
            .unwrap_or("-"),
        data_postgres_url = args.data.postgres_url.as_deref().unwrap_or("-"),
        data_postgres_require_ssl = args.data.postgres_require_ssl,
        "aether-gateway started"
    );

    let data_config = args.data.to_config();
    let mut state = AppState::new_with_executor(
        args.upstream,
        control_url.map(ToOwned::to_owned),
        executor_url.map(ToOwned::to_owned),
    )?
    .with_data_config(data_config)?
    .with_video_task_truth_source_mode(args.video_task_truth_source_mode.into());
    if matches!(
        args.video_task_truth_source_mode,
        VideoTaskTruthSourceArg::RustAuthoritative
    ) {
        state = state.with_video_task_poller_config(
            std::time::Duration::from_millis(args.video_task_poller_interval_ms.max(1)),
            args.video_task_poller_batch_size.max(1),
        );
    }
    if let Some(path) = args
        .video_task_store_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        state = state.with_video_task_store_path(path)?;
    }
    if let Some(limit) = args.max_in_flight_requests.filter(|limit| *limit > 0) {
        state = state.with_request_concurrency_limit(limit);
    }
    if let Some(limit) = args.distributed_request_limit.filter(|limit| *limit > 0) {
        let redis_url = args
            .distributed_request_redis_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "AETHER_GATEWAY_DISTRIBUTED_REQUEST_REDIS_URL is required when distributed request limit is enabled",
                )
            })?;
        state =
            state.with_distributed_request_concurrency_gate(DistributedConcurrencyGate::new_redis(
                "gateway_requests_distributed",
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
    info!(
        has_data_backends = state.has_data_backends(),
        has_video_task_data_reader = state.has_video_task_data_reader(),
        "aether-gateway data layer configured"
    );
    let background_tasks = state.spawn_background_tasks();
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    let router = build_router_with_state(state);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    for handle in background_tasks {
        handle.abort();
    }
    Ok(())
}
