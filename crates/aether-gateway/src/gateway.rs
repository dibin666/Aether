#[path = "audit/mod.rs"]
mod audit;
#[path = "cache/mod.rs"]
mod cache;
#[path = "constants.rs"]
mod constants;
#[path = "control.rs"]
mod control;
#[path = "data/mod.rs"]
mod data;
#[path = "error.rs"]
mod error;
#[path = "executor.rs"]
mod executor;
#[path = "handlers.rs"]
mod handlers;
#[path = "headers.rs"]
mod headers;
#[path = "kiro_stream.rs"]
mod kiro_stream;
#[path = "local_finalize.rs"]
mod local_finalize;
#[path = "local_stream.rs"]
mod local_stream;
#[path = "response.rs"]
mod response;
#[path = "video_tasks.rs"]
mod video_tasks;

use aether_contracts::ExecutionResult;
use aether_http::{build_http_client, HttpClientConfig};
use aether_runtime::{
    prometheus_response, service_up_sample, AdmissionPermit, ConcurrencyError, ConcurrencyGate,
    ConcurrencySnapshot, DistributedConcurrencyError, DistributedConcurrencyGate,
    DistributedConcurrencySnapshot, MetricKind, MetricLabel, MetricSample,
};
use axum::http::header::{HeaderName, HeaderValue};
use axum::routing::{any, get};
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::warn;

use cache::{AuthContextCache, DirectPlanBypassCache};

pub(crate) use audit::record_shadow_result_non_blocking;
use audit::{
    get_auth_api_key_snapshot, get_decision_trace, get_request_audit_bundle,
    get_request_candidate_trace, get_request_usage_audit, list_recent_shadow_results,
};
pub(crate) use control::{
    cache_executor_auth_context, maybe_execute_via_control, resolve_control_route,
    resolve_executor_auth_context, trusted_auth_local_rejection, GatewayControlAuthContext,
    GatewayControlDecision, GatewayLocalAuthRejection,
};
pub use data::GatewayDataConfig;
use data::GatewayDataState;
pub(crate) use error::GatewayError;
pub(crate) use executor::{maybe_execute_via_executor_stream, maybe_execute_via_executor_sync};
use handlers::{health, proxy_request};
pub(crate) use response::{
    attach_control_metadata_headers, build_client_response, build_client_response_from_parts,
    build_local_auth_rejection_response, build_local_overloaded_response,
};
pub(crate) use video_tasks::VideoTaskService;
pub use video_tasks::VideoTaskTruthSourceMode;

#[derive(Debug, Clone)]
pub struct AppState {
    upstream_base_url: String,
    control_base_url: Option<String>,
    executor_base_url: Option<String>,
    data: Arc<GatewayDataState>,
    video_tasks: Arc<VideoTaskService>,
    video_task_poller: Option<VideoTaskPollerConfig>,
    request_gate: Option<Arc<ConcurrencyGate>>,
    distributed_request_gate: Option<Arc<DistributedConcurrencyGate>>,
    client: reqwest::Client,
    auth_context_cache: Arc<AuthContextCache>,
    direct_plan_bypass_cache: Arc<DirectPlanBypassCache>,
}

#[derive(Debug, Clone, Copy)]
pub struct VideoTaskPollerConfig {
    interval: Duration,
    batch_size: usize,
}

impl AppState {
    pub fn new(
        upstream_base_url: impl Into<String>,
        control_base_url: Option<String>,
    ) -> Result<Self, reqwest::Error> {
        Self::new_with_executor(upstream_base_url, control_base_url, None)
    }

    pub fn new_with_executor(
        upstream_base_url: impl Into<String>,
        control_base_url: Option<String>,
        executor_base_url: Option<String>,
    ) -> Result<Self, reqwest::Error> {
        let client = build_http_client(&HttpClientConfig {
            connect_timeout_ms: Some(10_000),
            request_timeout_ms: Some(300_000),
            http2_adaptive_window: true,
            ..HttpClientConfig::default()
        })?;
        Ok(Self {
            upstream_base_url: normalize_upstream_base_url(upstream_base_url.into()),
            control_base_url: control_base_url
                .map(normalize_upstream_base_url)
                .filter(|value| !value.is_empty()),
            executor_base_url: executor_base_url
                .map(normalize_upstream_base_url)
                .filter(|value| !value.is_empty()),
            data: Arc::new(GatewayDataState::disabled()),
            video_tasks: Arc::new(VideoTaskService::new(
                VideoTaskTruthSourceMode::PythonSyncReport,
            )),
            video_task_poller: None,
            request_gate: None,
            distributed_request_gate: None,
            client,
            auth_context_cache: Arc::new(AuthContextCache::default()),
            direct_plan_bypass_cache: Arc::new(DirectPlanBypassCache::default()),
        })
    }

    pub fn with_data_config(
        mut self,
        config: GatewayDataConfig,
    ) -> Result<Self, aether_data::DataLayerError> {
        self.data = Arc::new(GatewayDataState::from_config(config)?);
        Ok(self)
    }

    pub fn with_video_task_truth_source_mode(mut self, mode: VideoTaskTruthSourceMode) -> Self {
        self.video_tasks = Arc::new(VideoTaskService::new(mode));
        self
    }

    pub fn with_video_task_poller_config(mut self, interval: Duration, batch_size: usize) -> Self {
        self.video_task_poller = Some(VideoTaskPollerConfig {
            interval,
            batch_size: batch_size.max(1),
        });
        self
    }

    pub fn with_request_concurrency_limit(mut self, limit: usize) -> Self {
        self.request_gate = Some(Arc::new(ConcurrencyGate::new(
            "gateway_requests",
            limit.max(1),
        )));
        self
    }

    pub fn with_distributed_request_concurrency_gate(
        mut self,
        gate: DistributedConcurrencyGate,
    ) -> Self {
        self.distributed_request_gate = Some(Arc::new(gate));
        self
    }

    pub fn has_data_backends(&self) -> bool {
        self.data.has_backends()
    }

    pub(crate) fn request_concurrency_snapshot(&self) -> Option<ConcurrencySnapshot> {
        self.request_gate.as_ref().map(|gate| gate.snapshot())
    }

    pub(crate) async fn distributed_request_concurrency_snapshot(
        &self,
    ) -> Result<Option<DistributedConcurrencySnapshot>, DistributedConcurrencyError> {
        match self.distributed_request_gate.as_ref() {
            Some(gate) => gate.snapshot().await.map(Some),
            None => Ok(None),
        }
    }

    pub(crate) async fn metric_samples(&self) -> Vec<MetricSample> {
        let mut samples = vec![service_up_sample("aether-gateway")];
        if let Some(snapshot) = self.request_concurrency_snapshot() {
            samples.extend(snapshot.to_metric_samples("gateway_requests"));
        }
        if let Some(gate) = self.distributed_request_gate.as_ref() {
            match gate.snapshot().await {
                Ok(snapshot) => {
                    samples.extend(snapshot.to_metric_samples("gateway_requests_distributed"));
                }
                Err(_) => samples.push(
                    MetricSample::new(
                        "concurrency_unavailable",
                        "Whether the distributed concurrency gate is currently unavailable.",
                        MetricKind::Gauge,
                        1,
                    )
                    .with_labels(vec![MetricLabel::new(
                        "gate",
                        "gateway_requests_distributed",
                    )]),
                ),
            }
        }
        samples
    }

    pub(crate) async fn try_acquire_request_permit(
        &self,
    ) -> Result<Option<AdmissionPermit>, RequestAdmissionError> {
        let local = self
            .request_gate
            .as_ref()
            .map(|gate| gate.try_acquire())
            .transpose()
            .map_err(RequestAdmissionError::Local)?;
        let distributed = match self.distributed_request_gate.as_ref() {
            Some(gate) => Some(
                gate.try_acquire()
                    .await
                    .map_err(RequestAdmissionError::Distributed)?,
            ),
            None => None,
        };
        Ok(AdmissionPermit::from_parts(local, distributed))
    }

    pub fn has_auth_api_key_data_reader(&self) -> bool {
        self.data.has_auth_api_key_reader()
    }

    pub fn has_video_task_data_reader(&self) -> bool {
        self.data.has_video_task_reader()
    }

    pub fn has_request_candidate_data_reader(&self) -> bool {
        self.data.has_request_candidate_reader()
    }

    pub fn has_provider_catalog_data_reader(&self) -> bool {
        self.data.has_provider_catalog_reader()
    }

    pub fn has_usage_data_reader(&self) -> bool {
        self.data.has_usage_reader()
    }

    pub fn has_shadow_result_data_writer(&self) -> bool {
        self.data.has_shadow_result_writer()
    }

    pub fn has_shadow_result_data_reader(&self) -> bool {
        self.data.has_shadow_result_reader()
    }

    pub(crate) async fn read_data_backed_video_task_response(
        &self,
        route_family: Option<&str>,
        request_path: &str,
    ) -> Result<Option<video_tasks::LocalVideoTaskReadResponse>, GatewayError> {
        self.data
            .read_video_task_response(route_family, request_path)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_request_candidate_trace(
        &self,
        request_id: &str,
        attempted_only: bool,
    ) -> Result<Option<data::RequestCandidateTrace>, GatewayError> {
        self.data
            .read_request_candidate_trace(request_id, attempted_only)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_decision_trace(
        &self,
        request_id: &str,
        attempted_only: bool,
    ) -> Result<Option<data::DecisionTrace>, GatewayError> {
        self.data
            .read_decision_trace(request_id, attempted_only)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_request_usage_audit(
        &self,
        request_id: &str,
    ) -> Result<Option<data::RequestUsageAudit>, GatewayError> {
        self.data
            .read_request_usage_audit(request_id)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_request_audit_bundle(
        &self,
        request_id: &str,
        attempted_only: bool,
        now_unix_secs: u64,
    ) -> Result<Option<data::RequestAuditBundle>, GatewayError> {
        self.data
            .read_request_audit_bundle(request_id, attempted_only, now_unix_secs)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_auth_api_key_snapshot(
        &self,
        user_id: &str,
        api_key_id: &str,
        now_unix_secs: u64,
    ) -> Result<Option<data::StoredGatewayAuthApiKeySnapshot>, GatewayError> {
        self.data
            .read_auth_api_key_snapshot(user_id, api_key_id, now_unix_secs)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn record_shadow_result_sample(
        &self,
        sample: aether_data::repository::shadow_results::RecordShadowResultSample,
    ) -> Result<Option<aether_data::repository::shadow_results::StoredShadowResult>, GatewayError>
    {
        self.data
            .record_shadow_result_sample(sample)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn list_recent_shadow_results(
        &self,
        limit: usize,
    ) -> Result<Vec<aether_data::repository::shadow_results::StoredShadowResult>, GatewayError>
    {
        self.data
            .list_recent_shadow_results(limit)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    #[cfg(test)]
    pub(crate) fn with_video_task_data_reader_for_tests(
        mut self,
        repository: Arc<dyn aether_data::repository::video_tasks::VideoTaskReadRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_video_task_reader_for_tests(
            repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_request_candidate_data_reader_for_tests(
        mut self,
        repository: Arc<dyn aether_data::repository::candidates::RequestCandidateReadRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_request_candidate_reader_for_tests(
            repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_decision_trace_data_readers_for_tests(
        mut self,
        request_candidate_repository: Arc<
            dyn aether_data::repository::candidates::RequestCandidateReadRepository,
        >,
        provider_catalog_repository: Arc<
            dyn aether_data::repository::provider_catalog::ProviderCatalogReadRepository,
        >,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_decision_trace_readers_for_tests(
            request_candidate_repository,
            provider_catalog_repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_request_audit_data_readers_for_tests(
        mut self,
        auth_api_key_repository: Arc<dyn aether_data::repository::auth::AuthApiKeyReadRepository>,
        request_candidate_repository: Arc<
            dyn aether_data::repository::candidates::RequestCandidateReadRepository,
        >,
        provider_catalog_repository: Arc<
            dyn aether_data::repository::provider_catalog::ProviderCatalogReadRepository,
        >,
        usage_repository: Arc<dyn aether_data::repository::usage::UsageReadRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_request_audit_readers_for_tests(
            auth_api_key_repository,
            request_candidate_repository,
            provider_catalog_repository,
            usage_repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_auth_api_key_data_reader_for_tests(
        mut self,
        repository: Arc<dyn aether_data::repository::auth::AuthApiKeyReadRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_auth_api_key_reader_for_tests(
            repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_usage_data_reader_for_tests(
        mut self,
        repository: Arc<dyn aether_data::repository::usage::UsageReadRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_usage_reader_for_tests(repository));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_shadow_result_data_writer_for_tests(
        mut self,
        repository: Arc<dyn aether_data::repository::shadow_results::ShadowResultWriteRepository>,
    ) -> Self {
        self.data = Arc::new(GatewayDataState::with_shadow_result_writer_for_tests(
            repository,
        ));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_shadow_result_data_repository_for_tests<T>(
        mut self,
        repository: Arc<T>,
    ) -> Self
    where
        T: aether_data::repository::shadow_results::ShadowResultRepository + 'static,
    {
        self.data = Arc::new(GatewayDataState::with_shadow_result_repository_for_tests(
            repository,
        ));
        self
    }

    pub fn with_video_task_store_path(
        mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<Self> {
        self.video_tasks = Arc::new(VideoTaskService::with_file_store(
            self.video_tasks.truth_source_mode(),
            path,
        )?);
        Ok(self)
    }

    pub fn spawn_background_tasks(&self) -> Vec<JoinHandle<()>> {
        let mut tasks = Vec::new();
        if let Some(handle) = self.spawn_video_task_poller() {
            tasks.push(handle);
        }
        tasks
    }

    pub(crate) async fn execute_video_task_refresh_plan(
        &self,
        executor_base_url: &str,
        refresh_plan: &video_tasks::LocalVideoTaskReadRefreshPlan,
    ) -> Result<bool, GatewayError> {
        let response = match self
            .client
            .post(format!("{executor_base_url}/v1/execute/sync"))
            .json(&refresh_plan.plan)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                warn!(error = %err, "gateway local video task refresh executor unavailable");
                return Ok(false);
            }
        };

        if response.status() != http::StatusCode::OK {
            return Ok(false);
        }

        let result: ExecutionResult = response
            .json()
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if result.status_code >= 400 {
            return Ok(false);
        }
        let provider_body = result
            .body
            .and_then(|body| body.json_body)
            .and_then(|body| body.as_object().cloned());
        let Some(provider_body) = provider_body else {
            return Ok(false);
        };

        Ok(self
            .video_tasks
            .apply_read_refresh_projection(refresh_plan, &provider_body))
    }

    async fn poll_video_tasks_once(&self, batch_size: usize) -> Result<usize, GatewayError> {
        if !self.video_tasks.is_rust_authoritative() {
            return Ok(0);
        }
        let Some(executor_base_url) = self.executor_base_url.as_deref() else {
            return Ok(0);
        };

        let refresh_plans = self
            .video_tasks
            .prepare_poll_refresh_batch(batch_size, "video-task-poller");
        let mut refreshed = 0usize;
        for refresh_plan in refresh_plans {
            if self
                .execute_video_task_refresh_plan(executor_base_url, &refresh_plan)
                .await?
            {
                refreshed += 1;
            }
        }
        Ok(refreshed)
    }

    fn spawn_video_task_poller(&self) -> Option<JoinHandle<()>> {
        let config = self.video_task_poller?;
        if !self.video_tasks.is_rust_authoritative() || self.executor_base_url.is_none() {
            return None;
        }

        let state = self.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(err) = state.poll_video_tasks_once(config.batch_size).await {
                    warn!(error = ?err, "gateway video task poller tick failed");
                }
            }
        }))
    }
}

pub fn build_router(upstream_base_url: impl Into<String>) -> Result<Router, reqwest::Error> {
    build_router_with_control(upstream_base_url, None)
}

pub fn build_router_with_control(
    upstream_base_url: impl Into<String>,
    control_base_url: Option<String>,
) -> Result<Router, reqwest::Error> {
    Ok(build_router_with_state(AppState::new(
        upstream_base_url,
        control_base_url,
    )?))
}

pub fn build_router_with_endpoints(
    upstream_base_url: impl Into<String>,
    control_base_url: Option<String>,
    executor_base_url: Option<String>,
) -> Result<Router, reqwest::Error> {
    Ok(build_router_with_state(AppState::new_with_executor(
        upstream_base_url,
        control_base_url,
        executor_base_url,
    )?))
}

pub fn build_router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/_gateway/health", get(health))
        .route("/_gateway/metrics", get(metrics))
        .route(
            "/_gateway/audit/auth/users/{user_id}/api-keys/{api_key_id}",
            get(get_auth_api_key_snapshot),
        )
        .route(
            "/_gateway/audit/decision-trace/{request_id}",
            get(get_decision_trace),
        )
        .route(
            "/_gateway/audit/request-candidates/{request_id}",
            get(get_request_candidate_trace),
        )
        .route(
            "/_gateway/audit/request-audit/{request_id}",
            get(get_request_audit_bundle),
        )
        .route(
            "/_gateway/audit/request-usage/{request_id}",
            get(get_request_usage_audit),
        )
        .route(
            "/_gateway/audit/shadow-results/recent",
            get(list_recent_shadow_results),
        )
        .route("/", any(proxy_request))
        .route("/{*path}", any(proxy_request))
        .with_state(state)
}

async fn metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    prometheus_response(&state.metric_samples().await)
}

#[derive(Debug)]
pub(crate) enum RequestAdmissionError {
    Local(ConcurrencyError),
    Distributed(DistributedConcurrencyError),
}

pub async fn serve_tcp(
    bind: &str,
    upstream_base_url: &str,
    control_base_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    serve_tcp_with_endpoints(bind, upstream_base_url, control_base_url, None).await
}

pub async fn serve_tcp_with_endpoints(
    bind: &str,
    upstream_base_url: &str,
    control_base_url: Option<&str>,
    executor_base_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let router = build_router_with_endpoints(
        upstream_base_url.to_string(),
        control_base_url.map(ToOwned::to_owned),
        executor_base_url.map(ToOwned::to_owned),
    )?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn normalize_upstream_base_url(upstream_base_url: String) -> String {
    upstream_base_url.trim_end_matches('/').to_string()
}

fn insert_header_if_missing(
    headers: &mut http::HeaderMap,
    key: &'static str,
    value: &str,
) -> Result<(), GatewayError> {
    if headers.contains_key(key) {
        return Ok(());
    }
    let name = HeaderName::from_static(key);
    let value =
        HeaderValue::from_str(value).map_err(|err| GatewayError::Internal(err.to_string()))?;
    headers.insert(name, value);
    Ok(())
}

#[cfg(test)]
mod tests;
