use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_data_contracts::repository::provider_key_task_events::ProviderKeyTaskEvent;
use futures_util::{stream, StreamExt};
use serde_json::{Map, Value};
use tokio::sync::Semaphore;
use tracing::{info, warn};

use crate::admin_api::provider_oauth_maintenance_endpoint_for_provider;
use crate::provider_key_auth::provider_key_is_oauth_managed;
use crate::task_runtime::{
    append_event_with_logging, ensure_worker_execution_run, TASK_KEY_OAUTH_TOKEN_REFRESH,
};
use crate::{AppState, GatewayError};

use super::{system_config_bool, system_config_u64, system_config_usize};

const OAUTH_TOKEN_REFRESH_DEFAULT_LOOKAHEAD_SECS: u64 = 120;
const OAUTH_TOKEN_REFRESH_DEFAULT_INTERVAL_SECS: u64 = 60;
const OAUTH_TOKEN_REFRESH_MIN_INTERVAL_SECS: u64 = 15;
const OAUTH_TOKEN_REFRESH_DEFAULT_CONCURRENCY: usize = 4;
const OAUTH_TOKEN_REFRESH_DEFAULT_MAX_PER_RUN: usize = 50;
const OAUTH_TOKEN_REFRESH_ACCOUNT_EVENT_LIMIT: usize = 200;
const OAUTH_TOKEN_REFRESH_PROVIDER_STAMP_PREFIX: &str = "ap:oauth_refresh:last_scan";
const OAUTH_ACCOUNT_BLOCK_PREFIX: &str = "[ACCOUNT_BLOCK] ";
const OAUTH_EXPIRED_PREFIX: &str = "[OAUTH_EXPIRED] ";
const OAUTH_REFRESH_FAILED_PREFIX: &str = "[REFRESH_FAILED] ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OAuthTokenRefreshWorkerConfig {
    pub(crate) lookahead_seconds: u64,
    pub(crate) interval: Duration,
    pub(crate) concurrency: usize,
    pub(crate) max_per_run: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthTokenRefreshProviderConfig {
    enabled: bool,
    lookahead_seconds: u64,
    scan_interval: Option<Duration>,
    concurrency: usize,
    max_per_run: usize,
    proxy_node_id_override: Option<Option<String>>,
}

impl OAuthTokenRefreshWorkerConfig {
    async fn load(state: &AppState) -> Result<Self, GatewayError> {
        let lookahead_seconds = system_config_u64(
            &state.data,
            "oauth_token_refresh_lookahead_seconds",
            OAUTH_TOKEN_REFRESH_DEFAULT_LOOKAHEAD_SECS,
        )
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
        .min(30 * 24 * 60 * 60);
        let interval_seconds = system_config_u64(
            &state.data,
            "oauth_token_refresh_interval_seconds",
            OAUTH_TOKEN_REFRESH_DEFAULT_INTERVAL_SECS,
        )
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
        .clamp(OAUTH_TOKEN_REFRESH_MIN_INTERVAL_SECS, 24 * 60 * 60);
        let concurrency = system_config_usize(
            &state.data,
            "oauth_token_refresh_concurrency",
            OAUTH_TOKEN_REFRESH_DEFAULT_CONCURRENCY,
        )
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
        .clamp(1, 64);
        let max_per_run = system_config_usize(
            &state.data,
            "oauth_token_refresh_max_per_run",
            OAUTH_TOKEN_REFRESH_DEFAULT_MAX_PER_RUN,
        )
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
        .clamp(1, 10_000);

        Ok(Self {
            lookahead_seconds,
            interval: Duration::from_secs(interval_seconds),
            concurrency,
            max_per_run,
        })
    }
}

impl OAuthTokenRefreshProviderConfig {
    fn from_provider_config(
        global: &OAuthTokenRefreshWorkerConfig,
        provider_config: Option<&Value>,
    ) -> Self {
        let config = oauth_token_refresh_provider_config_object(provider_config);
        let enabled = config
            .and_then(|object| provider_config_bool(object, "enabled"))
            .unwrap_or(true);
        let lookahead_seconds = config
            .and_then(|object| provider_config_u64(object, "lookahead_seconds"))
            .unwrap_or(global.lookahead_seconds)
            .min(30 * 24 * 60 * 60);
        let scan_interval = config
            .and_then(|object| provider_config_u64(object, "interval_seconds"))
            .map(|seconds| {
                Duration::from_secs(
                    seconds.clamp(OAUTH_TOKEN_REFRESH_MIN_INTERVAL_SECS, 24 * 60 * 60),
                )
            });
        let concurrency = config
            .and_then(|object| provider_config_usize(object, "concurrency"))
            .unwrap_or(global.concurrency)
            .clamp(1, 64);
        let max_per_run = config
            .and_then(|object| provider_config_usize(object, "max_per_run"))
            .unwrap_or(global.max_per_run)
            .clamp(1, 10_000);
        let proxy_node_id_override = provider_config_proxy_override(config);

        Self {
            enabled,
            lookahead_seconds,
            scan_interval,
            concurrency,
            max_per_run,
            proxy_node_id_override,
        }
    }
}

fn oauth_token_refresh_provider_config_object(
    provider_config: Option<&Value>,
) -> Option<&Map<String, Value>> {
    provider_config
        .and_then(Value::as_object)
        .and_then(|object| object.get("oauth_token_refresh"))
        .and_then(Value::as_object)
}

fn provider_config_bool(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

fn provider_config_u64(object: &Map<String, Value>, key: &str) -> Option<u64> {
    let value = object.get(key)?;
    if value.is_null() {
        return None;
    }
    value
        .as_u64()
        .or_else(|| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && *number >= 0.0)
                .map(|number| number as u64)
        })
        .or_else(|| {
            value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .and_then(|raw| raw.parse::<u64>().ok())
        })
}

fn provider_config_usize(object: &Map<String, Value>, key: &str) -> Option<usize> {
    provider_config_u64(object, key).and_then(|value| usize::try_from(value).ok())
}

fn provider_config_proxy_override(object: Option<&Map<String, Value>>) -> Option<Option<String>> {
    let object = object?;
    if !object.contains_key("proxy_node_id") {
        return None;
    }
    let value = object.get("proxy_node_id")?;
    if value.is_null() {
        return Some(None);
    }
    let proxy_node_id = value
        .as_str()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(ToOwned::to_owned);
    Some(proxy_node_id)
}

fn oauth_token_refresh_provider_scan_stamp_key(provider_id: &str) -> String {
    format!("{OAUTH_TOKEN_REFRESH_PROVIDER_STAMP_PREFIX}:{provider_id}")
}

fn oauth_token_refresh_scan_is_due(
    last_scan: u64,
    interval: Duration,
    now_ts: u64,
    invocation: OAuthTokenRefreshInvocation,
) -> bool {
    matches!(invocation, OAuthTokenRefreshInvocation::Manual)
        || last_scan == 0
        || now_ts >= last_scan.saturating_add(interval.as_secs())
}

async fn oauth_token_refresh_provider_scan_due(
    state: &AppState,
    provider_id: &str,
    provider_config: &OAuthTokenRefreshProviderConfig,
    now_ts: u64,
    invocation: OAuthTokenRefreshInvocation,
) -> bool {
    let Some(interval) = provider_config.scan_interval else {
        return true;
    };
    let key = oauth_token_refresh_provider_scan_stamp_key(provider_id);
    let last_scan = state
        .runtime_state
        .kv_get(&key)
        .await
        .ok()
        .flatten()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or_default();
    if !oauth_token_refresh_scan_is_due(last_scan, interval, now_ts, invocation) {
        return false;
    }
    let ttl = interval
        .checked_mul(2)
        .unwrap_or(interval)
        .saturating_add(Duration::from_secs(60));
    if let Err(err) = state
        .runtime_state
        .kv_set(&key, now_ts.to_string(), Some(ttl))
        .await
    {
        warn!(
            provider_id,
            error = ?err,
            "gateway oauth token refresh failed to record provider scan stamp"
        );
    }
    true
}

pub(crate) async fn oauth_token_refresh_interval(state: &AppState) -> Duration {
    OAuthTokenRefreshWorkerConfig::load(state)
        .await
        .map(|config| config.interval)
        .unwrap_or_else(|_| Duration::from_secs(OAUTH_TOKEN_REFRESH_DEFAULT_INTERVAL_SECS))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub(crate) struct OAuthTokenRefreshRunSummary {
    pub(crate) scanned: usize,
    pub(crate) eligible: usize,
    pub(crate) refreshed: usize,
    pub(crate) resolved: usize,
    pub(crate) skipped: usize,
    pub(crate) failed: usize,
}

struct OAuthTokenRefreshCandidate {
    provider_id: String,
    provider_name: String,
    provider_type: String,
    key_id: String,
    key_name: String,
    key: StoredProviderCatalogKey,
    transport: crate::provider_transport::GatewayProviderTransportSnapshot,
    proxy_node_id_override: Option<Option<String>>,
    provider_concurrency: usize,
}

enum OAuthTokenRefreshCandidateOutcome {
    Resolved {
        provider_id: String,
        provider_name: String,
        provider_type: String,
        key_id: String,
        key_name: String,
        refreshed: bool,
    },
    Skipped {
        provider_id: String,
        provider_name: String,
        provider_type: String,
        key_id: String,
        key_name: String,
        reason: String,
    },
    Failed {
        provider_id: String,
        provider_name: String,
        provider_type: String,
        key_id: String,
        key_name: String,
        error: String,
    },
}

impl OAuthTokenRefreshCandidateOutcome {
    fn provider_id(&self) -> &str {
        match self {
            Self::Resolved { provider_id, .. }
            | Self::Skipped { provider_id, .. }
            | Self::Failed { provider_id, .. } => provider_id,
        }
    }

    fn provider_name(&self) -> &str {
        match self {
            Self::Resolved { provider_name, .. }
            | Self::Skipped { provider_name, .. }
            | Self::Failed { provider_name, .. } => provider_name,
        }
    }

    fn provider_type(&self) -> &str {
        match self {
            Self::Resolved { provider_type, .. }
            | Self::Skipped { provider_type, .. }
            | Self::Failed { provider_type, .. } => provider_type,
        }
    }

    fn key_id(&self) -> &str {
        match self {
            Self::Resolved { key_id, .. }
            | Self::Skipped { key_id, .. }
            | Self::Failed { key_id, .. } => key_id,
        }
    }

    fn key_name(&self) -> &str {
        match self {
            Self::Resolved { key_name, .. }
            | Self::Skipped { key_name, .. }
            | Self::Failed { key_name, .. } => key_name,
        }
    }
}

struct OAuthTokenRefreshAccountEventBuilder<'a> {
    task_run_id: &'a str,
    event_name: &'a str,
    provider_id: &'a str,
    provider_name: &'a str,
    provider_type: &'a str,
    key_id: &'a str,
    key_name: &'a str,
    status: &'a str,
    message: &'a str,
    reason: Option<&'a str>,
    timestamp: u64,
}

impl<'a> OAuthTokenRefreshAccountEventBuilder<'a> {
    fn from_outcome(
        outcome: &'a OAuthTokenRefreshCandidateOutcome,
        task_run_id: &'a str,
        timestamp: u64,
    ) -> Self {
        let (event_name, status, message, reason) = match outcome {
            OAuthTokenRefreshCandidateOutcome::Resolved {
                refreshed: true, ..
            } => (
                "oauth_refresh_account_refreshed",
                "refreshed",
                "Token 已刷新",
                None,
            ),
            OAuthTokenRefreshCandidateOutcome::Resolved {
                refreshed: false, ..
            } => (
                "oauth_refresh_account_checked",
                "checked",
                "Token 已检查，无需更新",
                None,
            ),
            OAuthTokenRefreshCandidateOutcome::Skipped { reason, .. } => (
                "oauth_refresh_account_skipped",
                "skipped",
                "Token 刷新已跳过",
                Some(reason.as_str()),
            ),
            OAuthTokenRefreshCandidateOutcome::Failed { error, .. } => (
                "oauth_refresh_failed",
                "failed",
                "Token 刷新失败",
                Some(error.as_str()),
            ),
        };

        Self {
            task_run_id,
            event_name,
            provider_id: outcome.provider_id(),
            provider_name: outcome.provider_name(),
            provider_type: outcome.provider_type(),
            key_id: outcome.key_id(),
            key_name: outcome.key_name(),
            status,
            message,
            reason,
            timestamp,
        }
    }

    fn build(self) -> ProviderKeyTaskEvent {
        let mut event = ProviderKeyTaskEvent::new(
            TASK_KEY_OAUTH_TOKEN_REFRESH,
            self.task_run_id,
            self.event_name,
            self.provider_id,
            self.key_id,
            "oauth_refresh",
            self.status,
            self.timestamp,
        )
        .with_provider_name(Some(self.provider_name))
        .with_provider_type(Some(self.provider_type))
        .with_provider_api_key_name(Some(self.key_name))
        .with_message(Some(self.message));

        if let Some(reason) = self.reason {
            event = event.with_reason(Some(reason));
        }

        event
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OAuthTokenRefreshInvocation {
    Scheduled,
    Manual,
}

pub(crate) async fn perform_oauth_token_refresh_once(
    state: &AppState,
) -> Result<OAuthTokenRefreshRunSummary, GatewayError> {
    perform_oauth_token_refresh_once_with_invocation(state, OAuthTokenRefreshInvocation::Scheduled)
        .await
}

pub(crate) async fn perform_oauth_token_refresh_once_manual(
    state: &AppState,
) -> Result<OAuthTokenRefreshRunSummary, GatewayError> {
    perform_oauth_token_refresh_once_with_invocation(state, OAuthTokenRefreshInvocation::Manual)
        .await
}

async fn perform_oauth_token_refresh_once_with_invocation(
    state: &AppState,
    invocation: OAuthTokenRefreshInvocation,
) -> Result<OAuthTokenRefreshRunSummary, GatewayError> {
    if !state.has_provider_catalog_data_reader() || !state.has_provider_catalog_data_writer() {
        return Ok(OAuthTokenRefreshRunSummary::default());
    }
    if !system_config_bool(&state.data, "enable_oauth_token_refresh", true)
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
    {
        return Ok(OAuthTokenRefreshRunSummary::default());
    }

    let config = OAuthTokenRefreshWorkerConfig::load(state).await?;
    let now_ts = now_unix_secs();
    let task_run_id = ensure_worker_execution_run(state, TASK_KEY_OAUTH_TOKEN_REFRESH).await;

    let Some(catalog) = load_maintenance_catalog_snapshot(state).await? else {
        return Ok(OAuthTokenRefreshRunSummary::default());
    };

    let (candidates, mut summary) =
        collect_refresh_candidates(state, &catalog, &config, now_ts, invocation).await?;

    let outcomes = execute_refresh_candidates(state, candidates, config.concurrency).await;

    let account_events_recorded =
        process_outcomes(state, outcomes, task_run_id.as_deref(), &mut summary).await;

    record_run_completion(
        state,
        task_run_id.as_deref(),
        &config,
        &summary,
        account_events_recorded,
    )
    .await;

    Ok(summary)
}

struct MaintenanceCatalogSnapshot {
    providers: Vec<StoredProviderCatalogProvider>,
    endpoints_by_provider: BTreeMap<String, Vec<StoredProviderCatalogEndpoint>>,
    keys_by_provider: BTreeMap<String, Vec<StoredProviderCatalogKey>>,
}

async fn load_maintenance_catalog_snapshot(
    state: &AppState,
) -> Result<Option<MaintenanceCatalogSnapshot>, GatewayError> {
    // Maintenance must not let one malformed historical proxy credential
    // abort the scan for every provider. Read the rows first, then open each
    // row in isolation so a bad record can be skipped while database errors
    // and missing encryption configuration still fail closed.
    let providers = read_oauth_maintenance_providers(state).await?;
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    if provider_ids.is_empty() {
        return Ok(None);
    }

    let endpoints = read_oauth_maintenance_endpoints(state, &provider_ids).await?;
    // Read the catalog rows without opening/decrypting credentials in bulk.
    // A single legacy/plaintext row must not abort refresh for every healthy
    // key, and this maintenance scan must not trigger the normal lazy v2
    // credential rewrite path. Each candidate is opened in isolation below.
    let keys = state
        .data
        .list_provider_catalog_keys_by_provider_ids(&provider_ids)
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;

    Ok(Some(MaintenanceCatalogSnapshot {
        providers,
        endpoints_by_provider: group_by_provider_id(endpoints, |endpoint| &endpoint.provider_id),
        keys_by_provider: group_by_provider_id(keys, |key| &key.provider_id),
    }))
}

fn group_by_provider_id<T>(items: Vec<T>, get_id: impl Fn(&T) -> &str) -> BTreeMap<String, Vec<T>> {
    let mut grouped = BTreeMap::new();
    for item in items {
        grouped
            .entry(get_id(&item).to_owned())
            .or_insert_with(Vec::new)
            .push(item);
    }
    grouped
}

async fn read_oauth_maintenance_providers(
    state: &AppState,
) -> Result<Vec<StoredProviderCatalogProvider>, GatewayError> {
    let stored = state
        .data
        .list_provider_catalog_providers(true)
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let mut opened = Vec::with_capacity(stored.len());
    for provider in stored {
        let provider_id = provider.id.clone();
        match state
            .read_provider_catalog_providers_by_ids(std::slice::from_ref(&provider_id))
            .await
        {
            Ok(mut rows) => {
                if let Some(row) = rows.pop() {
                    opened.push(row);
                }
            }
            Err(error) if is_nonfatal_stored_proxy_error(&error) => {
                warn!(
                    event_name = "oauth_token_refresh_skipped_invalid_provider_proxy",
                    log_type = "ops",
                    worker = "oauth_token_refresh",
                    provider_id = %provider_id,
                    reason = "invalid_stored_proxy_credential",
                    "gateway skipped oauth refresh for a provider with an invalid stored proxy credential"
                );
            }
            Err(error) => return Err(error),
        }
    }
    Ok(opened)
}

async fn read_oauth_maintenance_endpoints(
    state: &AppState,
    provider_ids: &[String],
) -> Result<Vec<StoredProviderCatalogEndpoint>, GatewayError> {
    let stored = state
        .data
        .list_provider_catalog_endpoints_by_provider_ids(provider_ids)
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let mut opened = Vec::with_capacity(stored.len());
    for endpoint in stored {
        let provider_id = endpoint.provider_id.clone();
        let endpoint_id = endpoint.id.clone();
        match state
            .read_provider_catalog_endpoints_by_ids(std::slice::from_ref(&endpoint_id))
            .await
        {
            Ok(mut rows) => {
                if let Some(row) = rows.pop() {
                    opened.push(row);
                }
            }
            Err(error) if is_nonfatal_stored_proxy_error(&error) => {
                warn!(
                    event_name = "oauth_token_refresh_skipped_invalid_endpoint_proxy",
                    log_type = "ops",
                    worker = "oauth_token_refresh",
                    provider_id = %provider_id,
                    endpoint_id = %endpoint_id,
                    reason = "invalid_stored_proxy_credential",
                    "gateway skipped oauth refresh for an endpoint with an invalid stored proxy credential"
                );
            }
            Err(error) => return Err(error),
        }
    }
    Ok(opened)
}

async fn collect_refresh_candidates(
    state: &AppState,
    catalog: &MaintenanceCatalogSnapshot,
    config: &OAuthTokenRefreshWorkerConfig,
    now_ts: u64,
    invocation: OAuthTokenRefreshInvocation,
) -> Result<(Vec<OAuthTokenRefreshCandidate>, OAuthTokenRefreshRunSummary), GatewayError> {
    let mut summary = OAuthTokenRefreshRunSummary::default();
    let mut remaining_this_run = config.max_per_run;
    let mut candidates = Vec::<OAuthTokenRefreshCandidate>::new();

    'providers: for provider in &catalog.providers {
        if remaining_this_run == 0 {
            break;
        }
        let provider_config =
            OAuthTokenRefreshProviderConfig::from_provider_config(config, provider.config.as_ref());
        if !provider_config.enabled
            || !oauth_token_refresh_provider_scan_due(
                state,
                &provider.id,
                &provider_config,
                now_ts,
                invocation,
            )
            .await
        {
            continue;
        }
        let provider_keys = catalog
            .keys_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let provider_endpoints = catalog
            .endpoints_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let refresh_cutoff_unix_secs = now_ts.saturating_add(provider_config.lookahead_seconds);
        let provider_limit = provider_config.max_per_run.min(remaining_this_run);
        let mut provider_selected = 0usize;

        for key in provider_keys {
            summary.scanned = summary.scanned.saturating_add(1);
            match inspect_refresh_candidate(
                state,
                provider,
                provider_endpoints,
                &provider_config,
                key,
                refresh_cutoff_unix_secs,
            )
            .await?
            {
                Some(candidate) => {
                    summary.eligible = summary.eligible.saturating_add(1);
                    candidates.push(candidate);
                    provider_selected = provider_selected.saturating_add(1);
                    remaining_this_run = remaining_this_run.saturating_sub(1);
                    if remaining_this_run == 0 {
                        break 'providers;
                    }
                    if provider_selected >= provider_limit {
                        break;
                    }
                }
                None => {
                    summary.skipped = summary.skipped.saturating_add(1);
                }
            }
        }
    }

    Ok((candidates, summary))
}

async fn inspect_refresh_candidate(
    state: &AppState,
    provider: &StoredProviderCatalogProvider,
    provider_endpoints: &[StoredProviderCatalogEndpoint],
    provider_config: &OAuthTokenRefreshProviderConfig,
    key: &StoredProviderCatalogKey,
    refresh_cutoff_unix_secs: u64,
) -> Result<Option<OAuthTokenRefreshCandidate>, GatewayError> {
    if !oauth_refresh_candidate(provider, key) {
        return Ok(None);
    }

    let Some(endpoint) = provider_oauth_maintenance_endpoint_for_provider(
        &provider.provider_type,
        provider_endpoints,
    ) else {
        return Ok(None);
    };

    let transport = match state
        .read_provider_transport_snapshot(&provider.id, &endpoint.id, &key.id)
        .await
    {
        Ok(Some(transport)) => transport,
        Ok(None) => return Ok(None),
        Err(err) if is_nonfatal_legacy_catalog_credential_error(&err) => {
            // Keep malformed historical credentials untouched. They
            // are intentionally skipped while other keys continue.
            warn!(
                event_name = "oauth_token_refresh_skipped_invalid_credential",
                log_type = "ops",
                worker = "oauth_token_refresh",
                provider_id = %provider.id,
                key_id = %key.id,
                reason = "invalid_stored_credential",
                "gateway skipped oauth refresh for an invalid stored credential"
            );
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    let decrypted_auth_config = transport.key.decrypted_auth_config.as_deref();
    let is_agent_identity =
        crate::provider_transport::is_codex_agent_identity_transport(&transport);
    let needs_agent_task_recovery = is_agent_identity
        && agent_identity_needs_task_recovery(
            decrypted_auth_config,
            key.oauth_invalid_reason.as_deref(),
        );
    if !needs_agent_task_recovery && !auth_config_has_refresh_token(decrypted_auth_config) {
        return Ok(None);
    }
    if !needs_agent_task_recovery
        && !oauth_refresh_due_for_cutoff(key, decrypted_auth_config, refresh_cutoff_unix_secs)
    {
        return Ok(None);
    }

    Ok(Some(OAuthTokenRefreshCandidate {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        provider_type: provider.provider_type.clone(),
        key_id: key.id.clone(),
        key_name: key.name.clone(),
        key: key.clone(),
        transport,
        proxy_node_id_override: provider_config.proxy_node_id_override.clone(),
        provider_concurrency: provider_config.concurrency,
    }))
}

async fn execute_refresh_candidates(
    state: &AppState,
    candidates: Vec<OAuthTokenRefreshCandidate>,
    global_concurrency: usize,
) -> Vec<OAuthTokenRefreshCandidateOutcome> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut provider_limits = HashMap::<String, Arc<Semaphore>>::new();
    for candidate in &candidates {
        provider_limits
            .entry(candidate.provider_id.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(candidate.provider_concurrency)));
    }
    let execution_buffer = candidates.len().max(1);
    let global_limit = Arc::new(Semaphore::new(global_concurrency));
    stream::iter(candidates.into_iter().map(|candidate| {
        let global_limit = Arc::clone(&global_limit);
        let provider_limit = provider_limits
            .get(&candidate.provider_id)
            .cloned()
            .expect("oauth refresh provider semaphore missing");
        async move {
            let _global_permit = global_limit
                .acquire_owned()
                .await
                .expect("oauth refresh global semaphore closed");
            let _provider_permit = provider_limit
                .acquire_owned()
                .await
                .expect("oauth refresh provider semaphore closed");
            refresh_candidate(state, candidate).await
        }
    }))
    .buffer_unordered(execution_buffer)
    .collect::<Vec<_>>()
    .await
}

async fn refresh_candidate(
    state: &AppState,
    candidate: OAuthTokenRefreshCandidate,
) -> OAuthTokenRefreshCandidateOutcome {
    let refresh_result = state
        .force_local_oauth_refresh_entry_for_auto_refresh_with_proxy_override(
            &candidate.transport,
            candidate.proxy_node_id_override.clone(),
        )
        .await
        .map(|entry| entry.map(|_| ()));

    match refresh_result {
        Ok(Some(())) => match provider_key_credentials_changed(state, &candidate.key).await {
            Ok(refreshed) => OAuthTokenRefreshCandidateOutcome::Resolved {
                provider_id: candidate.provider_id,
                provider_name: candidate.provider_name,
                provider_type: candidate.provider_type,
                key_id: candidate.key_id,
                key_name: candidate.key_name,
                refreshed,
            },
            Err(err) => OAuthTokenRefreshCandidateOutcome::Failed {
                provider_id: candidate.provider_id,
                provider_name: candidate.provider_name,
                provider_type: candidate.provider_type,
                key_id: candidate.key_id,
                key_name: candidate.key_name,
                error: format!("{err:?}"),
            },
        },
        Ok(None) => OAuthTokenRefreshCandidateOutcome::Skipped {
            provider_id: candidate.provider_id,
            provider_name: candidate.provider_name,
            provider_type: candidate.provider_type,
            key_id: candidate.key_id,
            key_name: candidate.key_name,
            reason: "refresh_not_run".to_string(),
        },
        Err(err) => OAuthTokenRefreshCandidateOutcome::Failed {
            provider_id: candidate.provider_id,
            provider_name: candidate.provider_name,
            provider_type: candidate.provider_type,
            key_id: candidate.key_id,
            key_name: candidate.key_name,
            error: format!("{err:?}"),
        },
    }
}

async fn process_outcomes(
    state: &AppState,
    outcomes: Vec<OAuthTokenRefreshCandidateOutcome>,
    task_run_id: Option<&str>,
    summary: &mut OAuthTokenRefreshRunSummary,
) -> usize {
    let mut account_events_recorded = 0usize;
    for outcome in &outcomes {
        match outcome {
            OAuthTokenRefreshCandidateOutcome::Resolved { refreshed, .. } => {
                summary.resolved = summary.resolved.saturating_add(1);
                if *refreshed {
                    summary.refreshed = summary.refreshed.saturating_add(1);
                }
            }
            OAuthTokenRefreshCandidateOutcome::Skipped { .. } => {
                summary.skipped = summary.skipped.saturating_add(1);
            }
            OAuthTokenRefreshCandidateOutcome::Failed {
                provider_id,
                key_id,
                error,
                ..
            } => {
                summary.failed = summary.failed.saturating_add(1);
                warn!(
                    event_name = "oauth_token_refresh_failed",
                    log_type = "ops",
                    worker = "oauth_token_refresh",
                    provider_id = %provider_id,
                    key_id = %key_id,
                    error = %error,
                    "gateway oauth token auto refresh failed"
                );
            }
        }

        if let Some(task_run_id) = task_run_id {
            if account_events_recorded < OAUTH_TOKEN_REFRESH_ACCOUNT_EVENT_LIMIT {
                account_events_recorded = account_events_recorded.saturating_add(1);
                record_account_event(state, outcome, task_run_id).await;
            }
        }
    }
    account_events_recorded
}

async fn record_account_event(
    state: &AppState,
    outcome: &OAuthTokenRefreshCandidateOutcome,
    task_run_id: &str,
) {
    let event =
        OAuthTokenRefreshAccountEventBuilder::from_outcome(outcome, task_run_id, now_unix_secs())
            .build();
    if let Err(error) = state.append_provider_key_task_events(&[event]).await {
        let provider_id = outcome.provider_id();
        let key_id = outcome.key_id();
        warn!(
            event_name = "oauth_token_refresh_account_event_persistence_failed",
            log_type = "ops",
            worker = "oauth_token_refresh",
            provider_id = %provider_id,
            key_id = %key_id,
            error = ?error,
            "failed to persist oauth token refresh account event"
        );
    }
}

async fn record_run_completion(
    state: &AppState,
    task_run_id: Option<&str>,
    config: &OAuthTokenRefreshWorkerConfig,
    summary: &OAuthTokenRefreshRunSummary,
    account_events_recorded: usize,
) {
    info!(
        event_name = "oauth_token_refresh_completed",
        log_type = "ops",
        worker = "oauth_token_refresh",
        scanned = summary.scanned,
        eligible = summary.eligible,
        refreshed = summary.refreshed,
        resolved = summary.resolved,
        skipped = summary.skipped,
        failed = summary.failed,
        "gateway completed oauth token auto refresh scan"
    );
    if let Some(task_run_id) = task_run_id {
        append_event_with_logging(
            state,
            task_run_id,
            "oauth_refresh_completed",
            "oauth token refresh scan completed",
            Some(serde_json::json!({
                "scanned": summary.scanned,
                "eligible": summary.eligible,
                "resolved": summary.resolved,
                "refreshed": summary.refreshed,
                "skipped": summary.skipped,
                "failed": summary.failed,
                "lookahead_seconds": config.lookahead_seconds,
                "interval_seconds": config.interval.as_secs(),
                "concurrency": config.concurrency,
                "max_per_run": config.max_per_run,
                "account_events_recorded": account_events_recorded,
                "account_event_limit": OAUTH_TOKEN_REFRESH_ACCOUNT_EVENT_LIMIT,
            })),
        )
        .await;
    }
}

fn oauth_refresh_candidate(
    provider: &StoredProviderCatalogProvider,
    key: &StoredProviderCatalogKey,
) -> bool {
    let has_auth_config = key
        .encrypted_auth_config
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    // Catalog credentials are encrypted at this point. Defer exact Agent
    // Identity detection until the transport snapshot provides auth_config.
    let possible_agent_candidate = provider.provider_type.trim().eq_ignore_ascii_case("codex")
        && key.auth_type.trim().eq_ignore_ascii_case("oauth")
        && (key.expires_at_unix_secs.is_none()
            || key
                .oauth_invalid_reason
                .as_deref()
                .is_some_and(|reason| reason.contains(OAUTH_REFRESH_FAILED_PREFIX)));
    key.is_active
        && has_auth_config
        && (oauth_invalid_state_allows_refresh(key) || possible_agent_candidate)
        && provider_key_is_oauth_managed(key, provider.provider_type.as_str())
}

fn oauth_refresh_due_for_cutoff(
    key: &StoredProviderCatalogKey,
    auth_config: Option<&str>,
    refresh_cutoff_unix_secs: u64,
) -> bool {
    oauth_refresh_expires_at_unix_secs(key, auth_config)
        .is_some_and(|expires_at| expires_at <= refresh_cutoff_unix_secs)
}

fn oauth_refresh_expires_at_unix_secs(
    key: &StoredProviderCatalogKey,
    auth_config: Option<&str>,
) -> Option<u64> {
    if key.expires_at_unix_secs.is_some() {
        return key.expires_at_unix_secs;
    }

    let value = auth_config
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|auth_config| serde_json::from_str::<Value>(auth_config).ok())?;
    let object = value.as_object()?;
    for field in ["expires_at", "expiresAt", "expiry", "exp"] {
        if let Some(expires_at) = json_u64(object.get(field)) {
            return Some(expires_at);
        }
    }
    None
}

fn json_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    value
        .as_u64()
        .or_else(|| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && *number >= 0.0)
                .map(|number| number as u64)
        })
        .or_else(|| {
            value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .and_then(|raw| raw.parse::<u64>().ok())
        })
}

fn oauth_invalid_state_allows_refresh(key: &StoredProviderCatalogKey) -> bool {
    let Some(reason) = key
        .oauth_invalid_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return key.oauth_invalid_at_unix_secs.is_none();
    };

    let mut saw_oauth_expired = false;
    for line in reason.lines().map(str::trim) {
        if line.starts_with(OAUTH_REFRESH_FAILED_PREFIX)
            || line.starts_with(OAUTH_ACCOUNT_BLOCK_PREFIX)
        {
            return false;
        }
        if line.starts_with(OAUTH_EXPIRED_PREFIX) {
            saw_oauth_expired = true;
        }
    }

    saw_oauth_expired || key.oauth_invalid_at_unix_secs.is_none()
}
fn agent_identity_needs_task_recovery(
    auth_config: Option<&str>,
    oauth_invalid_reason: Option<&str>,
) -> bool {
    if oauth_invalid_reason.is_some_and(|reason| reason.contains(OAUTH_REFRESH_FAILED_PREFIX)) {
        return true;
    }
    auth_config
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .is_some_and(|config| {
            crate::provider_transport::is_codex_agent_identity_auth_config_value(&config)
                && !crate::provider_transport::codex_agent_identity_auth_config_has_task_id(&config)
        })
}
async fn provider_key_credentials_changed(
    state: &AppState,
    before: &StoredProviderCatalogKey,
) -> Result<bool, GatewayError> {
    let Some(after) = state
        .data
        .list_provider_catalog_keys_by_ids(std::slice::from_ref(&before.id))
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?
        .into_iter()
        .next()
    else {
        return Ok(false);
    };
    Ok(after.encrypted_api_key != before.encrypted_api_key
        || after.encrypted_auth_config != before.encrypted_auth_config
        || after.expires_at_unix_secs != before.expires_at_unix_secs)
}

fn auth_config_has_refresh_token(auth_config: Option<&str>) -> bool {
    let Some(auth_config) = auth_config.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(auth_config) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    ["refresh_token", "refreshToken"].iter().any(|field| {
        object
            .get(*field)
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    })
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Credential decoding errors are expected for rows written by older
/// versions of the service. They are non-fatal for a best-effort maintenance
/// scan, but normal request/admin paths still fail closed on the same error.
fn is_nonfatal_legacy_catalog_credential_error(error: &GatewayError) -> bool {
    is_nonfatal_legacy_provider_key_credential_error(error) || is_nonfatal_stored_proxy_error(error)
}

fn is_nonfatal_legacy_provider_key_credential_error(error: &GatewayError) -> bool {
    let GatewayError::Internal(message) = error else {
        return false;
    };
    let message = message.to_ascii_lowercase();
    // Missing encryption configuration is an operational failure and must
    // remain fail-closed.  Only errors that identify a stored field or a
    // malformed legacy ciphertext are safe to isolate to one key.
    if message.contains("encryption key is not configured") {
        return false;
    }
    message.contains("provider_api_keys.api_key")
        || message.contains("provider_api_keys.auth_config")
        || message.contains("provider_api_keys.api_formats")
        || message.contains("provider_api_keys.allowed_models")
        || message.contains("legacy provider catalog credential")
        || message.contains("stored provider catalog credential is empty")
        || message.contains("aether secret envelope has the wrong record binding")
        || message.contains("provider catalog credential is not an authenticated ciphertext")
        || message.contains("provider catalog credential contains reserved framing")
        || message.contains("provider catalog credential authentication failed")
        || message.contains("provider catalog credential envelope")
        || message
            .contains("provider catalog key provider binding changed during credential migration")
}

/// Stored provider/endpoint/key proxy secrets are opened independently by the
/// maintenance scan. A malformed historical row is safe to isolate, while
/// encryption/configuration failures remain fatal so operators are alerted.
fn is_nonfatal_stored_proxy_error(error: &GatewayError) -> bool {
    let GatewayError::Internal(message) = error else {
        return false;
    };
    let message = message.to_ascii_lowercase();
    message.contains("stored provider proxy credentials cannot be decrypted")
        || message.contains("stored endpoint proxy credentials cannot be decrypted")
        || message.contains("stored key proxy credentials cannot be decrypted")
        || message.contains("stored provider proxy changed during credential migration")
        || message.contains("stored endpoint proxy changed during credential migration")
        || message.contains("stored key changed during credential migration")
        || message.contains("stored provider proxy credential migration did not stabilize")
        || message.contains("stored endpoint proxy credential migration did not stabilize")
        || message.contains("stored key proxy credential migration did not stabilize")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use aether_crypto::{
        decrypt_python_fernet_ciphertext, encrypt_python_fernet_plaintext,
        DEVELOPMENT_ENCRYPTION_KEY,
    };
    use aether_data::repository::background_tasks::InMemoryBackgroundTaskRepository;
    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::background_tasks::BackgroundTaskListQuery;
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::json;

    use super::{
        agent_identity_needs_task_recovery, auth_config_has_refresh_token,
        is_nonfatal_legacy_catalog_credential_error, now_unix_secs, oauth_refresh_candidate,
        oauth_refresh_due_for_cutoff, oauth_token_refresh_scan_is_due, OAuthTokenRefreshInvocation,
        StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
        TASK_KEY_OAUTH_TOKEN_REFRESH,
    };
    use crate::GatewayError;

    fn sample_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-1".to_string(),
            "Codex".to_string(),
            None,
            "codex".to_string(),
        )
        .expect("provider should build")
    }

    fn sample_non_codex_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-1".to_string(),
            "OpenAI".to_string(),
            None,
            "openai".to_string(),
        )
        .expect("provider should build")
    }

    fn sample_oauth_key() -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "provider-1".to_string(),
            "OAuth".to_string(),
            "oauth".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            None,
            Some("encrypted-access-token".to_string()),
            Some("encrypted-auth-config".to_string()),
            None,
            None,
            None,
            Some(100),
            None,
            None,
        )
        .expect("transport fields should build")
    }

    #[test]
    fn oauth_refresh_candidate_allows_access_token_expired_invalid_state() {
        let provider = sample_provider();
        let mut key = sample_oauth_key();
        key.oauth_invalid_at_unix_secs = Some(90);
        key.oauth_invalid_reason = Some("[OAUTH_EXPIRED] access token invalid".to_string());

        assert!(oauth_refresh_candidate(&provider, &key));
    }

    #[test]
    fn oauth_refresh_candidate_skips_refresh_token_failure_invalid_state() {
        let provider = sample_non_codex_provider();
        let mut key = sample_oauth_key();
        key.oauth_invalid_at_unix_secs = Some(90);
        key.oauth_invalid_reason =
            Some("[REFRESH_FAILED] Token 续期失败 (401): refresh_token 无效".to_string());

        assert!(!oauth_refresh_candidate(&provider, &key));
    }

    #[test]
    fn oauth_refresh_candidate_allows_codex_refresh_failure_for_agent_recovery() {
        let provider = sample_provider();
        let mut key = sample_oauth_key();
        key.oauth_invalid_at_unix_secs = Some(90);
        key.oauth_invalid_reason = Some("[REFRESH_FAILED] Agent task missing".to_string());

        assert!(oauth_refresh_candidate(&provider, &key));
    }

    #[test]
    fn oauth_refresh_due_uses_decrypted_auth_config_expiry_fallback() {
        let mut key = sample_oauth_key();
        key.expires_at_unix_secs = None;
        let auth_config = r#"{"refresh_token":"refresh-token","expires_at":100}"#;

        assert!(oauth_refresh_due_for_cutoff(&key, Some(auth_config), 120));
    }

    #[test]
    fn oauth_refresh_due_when_one_day_twenty_one_hours_remain_in_three_day_window() {
        const NOW: u64 = 1_000_000;
        const ONE_DAY_TWENTY_ONE_HOURS: u64 = 24 * 60 * 60 + 21 * 60 * 60;
        const THREE_DAYS: u64 = 3 * 24 * 60 * 60;
        let mut key = sample_oauth_key();
        key.expires_at_unix_secs = Some(NOW + ONE_DAY_TWENTY_ONE_HOURS);

        assert!(oauth_refresh_due_for_cutoff(&key, None, NOW + THREE_DAYS,));
    }

    #[test]
    fn oauth_refresh_due_prefers_catalog_expiry_over_auth_config() {
        let mut key = sample_oauth_key();
        key.expires_at_unix_secs = Some(300);
        let auth_config = r#"{"refresh_token":"refresh-token","expires_at":100}"#;

        assert!(!oauth_refresh_due_for_cutoff(&key, Some(auth_config), 120));
    }

    #[test]
    fn legacy_antigravity_refresh_token_is_refreshable() {
        assert!(auth_config_has_refresh_token(Some(
            r#"{"refreshToken":"legacy-refresh-token"}"#,
        )));
    }

    #[test]
    fn expiring_antigravity_oauth_key_is_refresh_candidate() {
        let provider = StoredProviderCatalogProvider::new(
            "provider-antigravity".to_string(),
            "Antigravity".to_string(),
            None,
            "antigravity".to_string(),
        )
        .expect("provider should build");
        let mut key = StoredProviderCatalogKey::new(
            "key-antigravity".to_string(),
            provider.id.clone(),
            "Antigravity OAuth".to_string(),
            "oauth".to_string(),
            None,
            true,
        )
        .expect("key should build");
        key.encrypted_auth_config = Some("encrypted-auth-config".to_string());
        key.expires_at_unix_secs = Some(120);

        assert!(oauth_refresh_candidate(&provider, &key));
    }

    #[test]
    fn pending_agent_identity_without_task_is_recoverable() {
        let config = serde_json::json!({
            "auth_mode": "agentIdentity",
            "agent_runtime_id": "runtime-1",
            "agent_private_key": "private-key-present",
        });
        assert!(agent_identity_needs_task_recovery(
            Some(&config.to_string()),
            None,
        ));
    }

    #[test]
    fn refresh_failure_marker_forces_agent_task_recovery() {
        assert!(agent_identity_needs_task_recovery(
            Some("{}"),
            Some("[REFRESH_FAILED] temporary"),
        ));
    }

    #[test]
    fn manual_refresh_bypasses_provider_scan_cadence() {
        let interval = Duration::from_secs(60 * 60);
        assert!(!oauth_token_refresh_scan_is_due(
            10_000,
            interval,
            10_001,
            OAuthTokenRefreshInvocation::Scheduled,
        ));
        assert!(oauth_token_refresh_scan_is_due(
            10_000,
            interval,
            10_001,
            OAuthTokenRefreshInvocation::Manual,
        ));
    }

    #[tokio::test]
    async fn scheduled_worker_refreshes_and_persists_oauth_credentials_on_timer() {
        let refresh_hits = Arc::new(AtomicUsize::new(0));
        let refresh_hits_for_server = Arc::clone(&refresh_hits);
        let token_server = Router::new().route(
            "/oauth/token",
            post(move || {
                let hits = Arc::clone(&refresh_hits_for_server);
                async move {
                    let refresh_number = hits.fetch_add(1, Ordering::SeqCst) + 1;
                    Json(json!({
                        "access_token": format!("scheduled-access-token-{refresh_number}"),
                        "refresh_token": format!("scheduled-refresh-token-{refresh_number}"),
                        "expires_in": 1,
                        "token_type": "Bearer",
                    }))
                }
            }),
        );
        let listener = crate::test_support::bind_loopback_listener()
            .await
            .expect("token test server should bind");
        let token_addr = listener
            .local_addr()
            .expect("token test server address should resolve");
        let token_server_handle = tokio::spawn(async move {
            axum::serve(listener, token_server)
                .await
                .expect("token test server should run");
        });

        let provider = StoredProviderCatalogProvider::new(
            "provider-scheduled-oauth".to_string(),
            "Scheduled OAuth".to_string(),
            Some("https://chatgpt.com/backend-api/codex".to_string()),
            "codex".to_string(),
        )
        .expect("provider should build");
        let endpoint = StoredProviderCatalogEndpoint::new(
            "endpoint-scheduled-oauth".to_string(),
            provider.id.clone(),
            "openai:responses".to_string(),
            None,
            None,
            true,
        )
        .expect("endpoint should build")
        .with_transport_fields(
            "https://chatgpt.com/backend-api/codex".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("endpoint transport should build");

        let initial_expiry = now_unix_secs().saturating_add(1);
        let encrypted_api_key = encrypt_python_fernet_plaintext(
            DEVELOPMENT_ENCRYPTION_KEY,
            "scheduled-stale-access-token",
        )
        .expect("api key ciphertext should build");
        let encrypted_auth_config = encrypt_python_fernet_plaintext(
            DEVELOPMENT_ENCRYPTION_KEY,
            &json!({
                "provider_type": "codex",
                "access_token": "scheduled-stale-access-token",
                "refresh_token": "scheduled-stale-refresh-token",
                "expires_at": initial_expiry,
            })
            .to_string(),
        )
        .expect("auth config ciphertext should build");
        let mut key = StoredProviderCatalogKey::new(
            "key-scheduled-oauth".to_string(),
            provider.id.clone(),
            "Scheduled OAuth Account".to_string(),
            "oauth".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_transport_fields(
            Some(json!(["openai:responses"])),
            encrypted_api_key,
            Some(encrypted_auth_config),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("key transport should build");
        key.expires_at_unix_secs = Some(initial_expiry);

        let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![provider],
            vec![endpoint],
            vec![key],
        ));
        let background_task_repository = Arc::new(InMemoryBackgroundTaskRepository::default());
        let provider_key_task_event_repository =
            Arc::new(aether_data::repository::provider_key_task_events::InMemoryProviderKeyTaskEventRepository::new());
        let data = crate::data::GatewayDataState::with_provider_catalog_repository_for_tests(
            provider_catalog_repository,
        )
        .with_encryption_key_for_tests(DEVELOPMENT_ENCRYPTION_KEY)
        .with_background_task_repository_for_tests(background_task_repository)
        .with_provider_key_task_event_repository_for_tests(provider_key_task_event_repository)
        .with_system_config_values_for_tests([
            ("enable_oauth_token_refresh".to_string(), json!(true)),
            (
                "oauth_token_refresh_interval_seconds".to_string(),
                json!(15),
            ),
            (
                "oauth_token_refresh_lookahead_seconds".to_string(),
                json!(120),
            ),
        ]);
        let oauth_refresh =
            crate::provider_transport::LocalOAuthRefreshCoordinator::with_adapters_for_tests(vec![
                Arc::new(
                    crate::provider_transport::oauth_refresh::GenericOAuthRefreshAdapter::default()
                        .with_token_url_for_tests(
                            "codex",
                            format!("http://{token_addr}/oauth/token"),
                        ),
                ),
            ]);
        let state = crate::AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(data)
            .with_oauth_refresh_coordinator_for_tests(oauth_refresh);
        let observer = state.clone();

        let worker = crate::maintenance::spawn_oauth_token_refresh_worker(state)
            .expect("scheduled OAuth worker should start");

        tokio::time::timeout(Duration::from_secs(5), async {
            while refresh_hits.load(Ordering::SeqCst) < 1 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("worker should perform its startup scan automatically");

        let first_key = observer
            .list_provider_catalog_keys_by_ids(&["key-scheduled-oauth".to_string()])
            .await
            .expect("refreshed key should read")
            .into_iter()
            .next()
            .expect("refreshed key should exist");
        let first_access_token = crate::handlers::shared::open_provider_catalog_credential(
            &observer,
            &first_key.provider_id,
            &first_key.id,
            crate::handlers::shared::ProviderCatalogCredentialField::ApiKey,
            first_key
                .encrypted_api_key
                .as_deref()
                .expect("refreshed api key ciphertext should exist"),
        )
        .expect("refreshed api key should decrypt")
        .plaintext;
        assert_eq!(first_access_token, "scheduled-access-token-1");

        tokio::time::timeout(Duration::from_secs(18), async {
            while refresh_hits.load(Ordering::SeqCst) < 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("worker should perform another scan after its configured interval");

        let second_key = observer
            .list_provider_catalog_keys_by_ids(&["key-scheduled-oauth".to_string()])
            .await
            .expect("second refreshed key should read")
            .into_iter()
            .next()
            .expect("second refreshed key should exist");
        let second_access_token = crate::handlers::shared::open_provider_catalog_credential(
            &observer,
            &second_key.provider_id,
            &second_key.id,
            crate::handlers::shared::ProviderCatalogCredentialField::ApiKey,
            second_key
                .encrypted_api_key
                .as_deref()
                .expect("second refreshed api key ciphertext should exist"),
        )
        .expect("second refreshed api key should decrypt")
        .plaintext;
        assert_eq!(second_access_token, "scheduled-access-token-2");

        let runs = observer
            .list_background_task_runs(&BackgroundTaskListQuery {
                task_key_substring: Some(TASK_KEY_OAUTH_TOKEN_REFRESH.to_string()),
                offset: 0,
                limit: 10,
                ..BackgroundTaskListQuery::default()
            })
            .await
            .expect("scheduled task runs should read");
        let execution_run = runs
            .items
            .iter()
            .find(|run| run.id.starts_with("run:"))
            .expect("worker execution run should exist");
        let events = observer
            .list_background_task_events(&execution_run.id, 0, 100, false)
            .await
            .expect("scheduled task events should read");
        let completed_events = events
            .iter()
            .filter(|event| event.event_type == "oauth_refresh_completed")
            .count();
        assert!(
            completed_events >= 2,
            "both automatic scans should record completed summary events in background tasks"
        );
        let account_events = observer
            .list_provider_key_task_events(
                &aether_data_contracts::repository::provider_key_task_events::ProviderKeyTaskEventQuery::new(
                    TASK_KEY_OAUTH_TOKEN_REFRESH,
                ),
            )
            .await
            .expect("scheduled account task events should read");
        let account_refresh_events = account_events
            .iter()
            .filter(|event| event.event_type == "oauth_refresh_account_refreshed")
            .count();
        assert!(
            account_refresh_events >= 2,
            "both automatic scans should record account refresh events in provider key task events"
        );
        assert!(
            account_events.iter().any(|e| {
                e.provider_id == "provider-scheduled-oauth"
                    && e.provider_api_key_id == "key-scheduled-oauth"
                    && e.message.as_deref() == Some("Token 已刷新")
            }),
            "account events should retain provider/key identification and message"
        );
        assert!(
            events
                .iter()
                .all(|event| !event.event_type.starts_with("manual_")),
            "scheduled test must not rely on the manual trigger path"
        );

        worker.abort();
        token_server_handle.abort();
    }
    #[test]
    fn only_stored_catalog_credential_errors_are_non_fatal() {
        assert!(is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal(
                "provider catalog credential is not an authenticated ciphertext".to_string(),
            )
        ));
        assert!(is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal(
                "provider_api_keys.auth_config has an invalid provider catalog credential envelope"
                    .to_string(),
            )
        ));
        assert!(!is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal("postgres error: connection refused".to_string(),)
        ));
        assert!(!is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal(
                "provider catalog credential encryption key is not configured".to_string(),
            )
        ));
        for scope in ["provider", "endpoint", "key"] {
            assert!(is_nonfatal_legacy_catalog_credential_error(
                &GatewayError::Internal(format!(
                    "stored {scope} proxy credentials cannot be decrypted"
                ))
            ));
        }
        assert!(is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal("stored provider catalog credential is empty".to_string())
        ));
        assert!(is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal(
                "Aether secret envelope has the wrong record binding".to_string()
            )
        ));
        for field in ["api_formats", "allowed_models"] {
            assert!(is_nonfatal_legacy_catalog_credential_error(
                &GatewayError::Internal(format!(
                    "provider_api_keys.{field} contains a malformed value"
                ))
            ));
        }
        assert!(!is_nonfatal_legacy_catalog_credential_error(
            &GatewayError::Internal(
                "endpoint proxy credential encryption is unavailable".to_string(),
            )
        ));
    }
}
