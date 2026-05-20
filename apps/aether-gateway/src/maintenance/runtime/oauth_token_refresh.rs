use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aether_data_contracts::repository::{
    background_tasks::{BackgroundTaskKind, BackgroundTaskStatus, UpsertBackgroundTaskRun},
    provider_catalog::{
        StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
    },
};
use futures_util::{stream, StreamExt};
use serde_json::{Map, Value};
use tokio::sync::Semaphore;
use tracing::{info, warn};

use crate::admin_api::provider_oauth_maintenance_endpoint_for_provider;
use crate::provider_key_auth::provider_key_is_oauth_managed;
use crate::task_runtime::{
    append_event_with_logging, task_definition, upsert_run_with_logging,
    TASK_KEY_OAUTH_TOKEN_REFRESH,
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

async fn oauth_token_refresh_provider_scan_due(
    state: &AppState,
    provider_id: &str,
    provider_config: &OAuthTokenRefreshProviderConfig,
    now_ts: u64,
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
    if last_scan > 0 && now_ts < last_scan.saturating_add(interval.as_secs()) {
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

async fn ensure_oauth_token_refresh_run(state: &AppState, now_ts: u64) {
    if !state.has_background_task_data_writer() {
        return;
    }
    let run_id = oauth_token_refresh_run_id(state);
    if state
        .find_background_task_run(&run_id)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }
    let max_attempts = task_definition(TASK_KEY_OAUTH_TOKEN_REFRESH)
        .map(|item| item.retry_policy.max_attempts)
        .unwrap_or(1);
    let run = UpsertBackgroundTaskRun {
        id: run_id,
        task_key: TASK_KEY_OAUTH_TOKEN_REFRESH.to_string(),
        kind: BackgroundTaskKind::Scheduled,
        trigger: "interval".to_string(),
        status: BackgroundTaskStatus::Running,
        attempt: 1,
        max_attempts,
        owner_instance: Some(state.tunnel.local_instance_id().to_string()),
        progress_percent: 0,
        progress_message: Some("oauth token refresh worker running".to_string()),
        payload_json: None,
        result_json: None,
        error_message: None,
        cancel_requested: false,
        created_by: Some("system".to_string()),
        created_at_unix_secs: now_ts,
        started_at_unix_secs: Some(now_ts),
        finished_at_unix_secs: None,
        updated_at_unix_secs: now_ts,
    };
    let _ = upsert_run_with_logging(state, run).await;
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

pub(crate) async fn perform_oauth_token_refresh_once(
    state: &AppState,
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
    ensure_oauth_token_refresh_run(state, now_ts).await;
    let providers = state.list_provider_catalog_providers(true).await?;
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    if provider_ids.is_empty() {
        return Ok(OAuthTokenRefreshRunSummary::default());
    }

    let endpoints = state
        .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
        .await?;
    let keys = state
        .list_provider_catalog_keys_by_provider_ids(&provider_ids)
        .await?;
    let endpoints_by_provider = group_endpoints_by_provider(endpoints);
    let keys_by_provider = group_keys_by_provider(keys);
    let mut summary = OAuthTokenRefreshRunSummary::default();
    let mut remaining_this_run = config.max_per_run;
    let mut candidates = Vec::<OAuthTokenRefreshCandidate>::new();

    'providers: for provider in providers {
        if remaining_this_run == 0 {
            break;
        }
        let provider_config = OAuthTokenRefreshProviderConfig::from_provider_config(
            &config,
            provider.config.as_ref(),
        );
        if !provider_config.enabled
            || !oauth_token_refresh_provider_scan_due(state, &provider.id, &provider_config, now_ts)
                .await
        {
            continue;
        }
        let provider_keys = keys_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let provider_endpoints = endpoints_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let refresh_cutoff_unix_secs = now_ts.saturating_add(provider_config.lookahead_seconds);
        let provider_limit = provider_config.max_per_run.min(remaining_this_run);
        let mut provider_selected = 0usize;
        for key in provider_keys {
            summary.scanned = summary.scanned.saturating_add(1);
            if !oauth_refresh_candidate(&provider, key, refresh_cutoff_unix_secs) {
                summary.skipped = summary.skipped.saturating_add(1);
                continue;
            }
            summary.eligible = summary.eligible.saturating_add(1);

            let Some(endpoint) = provider_oauth_maintenance_endpoint_for_provider(
                &provider.provider_type,
                provider_endpoints,
            ) else {
                summary.skipped = summary.skipped.saturating_add(1);
                continue;
            };

            let Some(transport) = state
                .read_provider_transport_snapshot(&provider.id, &endpoint.id, &key.id)
                .await?
            else {
                summary.skipped = summary.skipped.saturating_add(1);
                continue;
            };
            if !auth_config_has_refresh_token(transport.key.decrypted_auth_config.as_deref()) {
                summary.skipped = summary.skipped.saturating_add(1);
                continue;
            }

            candidates.push(OAuthTokenRefreshCandidate {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                provider_type: provider.provider_type.clone(),
                key_id: key.id.clone(),
                key_name: key.name.clone(),
                key: key.clone(),
                transport,
                proxy_node_id_override: provider_config.proxy_node_id_override.clone(),
                provider_concurrency: provider_config.concurrency,
            });
            provider_selected = provider_selected.saturating_add(1);
            remaining_this_run = remaining_this_run.saturating_sub(1);
            if remaining_this_run == 0 {
                break 'providers;
            }
            if provider_selected >= provider_limit {
                break;
            }
        }
    }

    let mut provider_limits = HashMap::<String, Arc<Semaphore>>::new();
    for candidate in &candidates {
        provider_limits
            .entry(candidate.provider_id.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(candidate.provider_concurrency)));
    }
    let execution_buffer = candidates.len().max(1);
    let global_limit = Arc::new(Semaphore::new(config.concurrency));
    let outcomes = stream::iter(candidates.into_iter().map(|candidate| {
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
            let refreshed_entry = state
                .force_local_oauth_refresh_entry_for_auto_refresh_with_proxy_override(
                    &candidate.transport,
                    candidate.proxy_node_id_override.clone(),
                )
                .await;
            match refreshed_entry {
                Ok(Some(_entry)) => {
                    match provider_key_credentials_changed(state, &candidate.key).await {
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
                    }
                }
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
    }))
    .buffer_unordered(execution_buffer)
    .collect::<Vec<_>>()
    .await;

    let mut account_events_recorded = 0usize;
    for outcome in outcomes {
        match outcome {
            OAuthTokenRefreshCandidateOutcome::Resolved {
                provider_id,
                provider_name,
                provider_type,
                key_id,
                key_name,
                refreshed,
            } => {
                summary.resolved = summary.resolved.saturating_add(1);
                if refreshed {
                    summary.refreshed = summary.refreshed.saturating_add(1);
                }
                if account_events_recorded < OAUTH_TOKEN_REFRESH_ACCOUNT_EVENT_LIMIT {
                    account_events_recorded = account_events_recorded.saturating_add(1);
                    append_event_with_logging(
                        state,
                        &oauth_token_refresh_run_id(state),
                        if refreshed {
                            "oauth_refresh_account_refreshed"
                        } else {
                            "oauth_refresh_account_checked"
                        },
                        if refreshed {
                            "oauth token refreshed"
                        } else {
                            "oauth token checked"
                        },
                        Some(serde_json::json!({
                            "provider_id": provider_id,
                            "provider_name": provider_name,
                            "provider_type": provider_type,
                            "key_id": key_id,
                            "key_name": key_name,
                            "action": "oauth_refresh",
                            "status": if refreshed { "refreshed" } else { "checked" },
                            "message": if refreshed { "Token 已刷新" } else { "Token 已检查，无需更新" },
                            "refreshed": refreshed,
                        })),
                    )
                    .await;
                }
            }
            OAuthTokenRefreshCandidateOutcome::Skipped {
                provider_id,
                provider_name,
                provider_type,
                key_id,
                key_name,
                reason,
            } => {
                summary.skipped = summary.skipped.saturating_add(1);
                if account_events_recorded < OAUTH_TOKEN_REFRESH_ACCOUNT_EVENT_LIMIT {
                    account_events_recorded = account_events_recorded.saturating_add(1);
                    append_event_with_logging(
                        state,
                        &oauth_token_refresh_run_id(state),
                        "oauth_refresh_account_skipped",
                        "oauth token refresh skipped",
                        Some(serde_json::json!({
                            "provider_id": provider_id,
                            "provider_name": provider_name,
                            "provider_type": provider_type,
                            "key_id": key_id,
                            "key_name": key_name,
                            "action": "oauth_refresh",
                            "status": "skipped",
                            "message": "Token 刷新已跳过",
                            "reason": reason,
                        })),
                    )
                    .await;
                }
            }
            OAuthTokenRefreshCandidateOutcome::Failed {
                provider_id,
                provider_name,
                provider_type,
                key_id,
                key_name,
                error,
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
                account_events_recorded = account_events_recorded.saturating_add(1);
                append_event_with_logging(
                    state,
                    &oauth_token_refresh_run_id(state),
                    "oauth_refresh_failed",
                    "oauth token refresh failed",
                    Some(serde_json::json!({
                        "provider_id": provider_id,
                        "provider_name": provider_name,
                        "provider_type": provider_type,
                        "key_id": key_id,
                        "key_name": key_name,
                        "action": "oauth_refresh",
                        "status": "failed",
                        "message": "Token 刷新失败",
                        "error": error,
                    })),
                )
                .await;
            }
        }
    }

    if summary.eligible > 0 || summary.refreshed > 0 || summary.failed > 0 {
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
        append_event_with_logging(
            state,
            &oauth_token_refresh_run_id(state),
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

    Ok(summary)
}

fn oauth_token_refresh_run_id(state: &AppState) -> String {
    format!(
        "boot:{}:{}",
        TASK_KEY_OAUTH_TOKEN_REFRESH,
        state.tunnel.local_instance_id()
    )
}

fn group_endpoints_by_provider(
    endpoints: Vec<StoredProviderCatalogEndpoint>,
) -> BTreeMap<String, Vec<StoredProviderCatalogEndpoint>> {
    let mut grouped = BTreeMap::new();
    for endpoint in endpoints {
        grouped
            .entry(endpoint.provider_id.clone())
            .or_insert_with(Vec::new)
            .push(endpoint);
    }
    grouped
}

fn group_keys_by_provider(
    keys: Vec<StoredProviderCatalogKey>,
) -> BTreeMap<String, Vec<StoredProviderCatalogKey>> {
    let mut grouped = BTreeMap::new();
    for key in keys {
        grouped
            .entry(key.provider_id.clone())
            .or_insert_with(Vec::new)
            .push(key);
    }
    grouped
}

fn oauth_refresh_candidate(
    provider: &StoredProviderCatalogProvider,
    key: &StoredProviderCatalogKey,
    refresh_cutoff_unix_secs: u64,
) -> bool {
    key.is_active
        && oauth_invalid_state_allows_refresh(key)
        && key
            .encrypted_auth_config
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        && key
            .expires_at_unix_secs
            .is_some_and(|expires_at| expires_at <= refresh_cutoff_unix_secs)
        && provider_key_is_oauth_managed(key, provider.provider_type.as_str())
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

async fn provider_key_credentials_changed(
    state: &AppState,
    before: &StoredProviderCatalogKey,
) -> Result<bool, GatewayError> {
    let Some(after) = state
        .list_provider_catalog_keys_by_ids(std::slice::from_ref(&before.id))
        .await?
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
    value
        .as_object()
        .and_then(|object| object.get("refresh_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{oauth_refresh_candidate, StoredProviderCatalogKey, StoredProviderCatalogProvider};

    fn sample_provider() -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            "provider-1".to_string(),
            "Codex".to_string(),
            None,
            "codex".to_string(),
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

        assert!(oauth_refresh_candidate(&provider, &key, 120));
    }

    #[test]
    fn oauth_refresh_candidate_skips_refresh_token_failure_invalid_state() {
        let provider = sample_provider();
        let mut key = sample_oauth_key();
        key.oauth_invalid_at_unix_secs = Some(90);
        key.oauth_invalid_reason =
            Some("[REFRESH_FAILED] Token 续期失败 (401): refresh_token 无效".to_string());

        assert!(!oauth_refresh_candidate(&provider, &key, 120));
    }
}
