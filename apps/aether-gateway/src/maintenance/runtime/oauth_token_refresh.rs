use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use futures_util::{stream, StreamExt};
use serde_json::Value;
use tracing::{info, warn};

use crate::admin_api::provider_oauth_maintenance_endpoint_for_provider;
use crate::provider_key_auth::provider_key_is_oauth_managed;
use crate::task_runtime::{append_event_with_logging, TASK_KEY_OAUTH_TOKEN_REFRESH};
use crate::{AppState, GatewayError};

use super::{system_config_bool, system_config_u64, system_config_usize};

const OAUTH_TOKEN_REFRESH_DEFAULT_LOOKAHEAD_SECS: u64 = 120;
const OAUTH_TOKEN_REFRESH_DEFAULT_INTERVAL_SECS: u64 = 60;
const OAUTH_TOKEN_REFRESH_MIN_INTERVAL_SECS: u64 = 15;
const OAUTH_TOKEN_REFRESH_DEFAULT_CONCURRENCY: usize = 4;
const OAUTH_TOKEN_REFRESH_DEFAULT_MAX_PER_RUN: usize = 50;
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
    provider_type: String,
    key_id: String,
    key: StoredProviderCatalogKey,
    transport: crate::provider_transport::GatewayProviderTransportSnapshot,
}

enum OAuthTokenRefreshCandidateOutcome {
    Resolved {
        refreshed: bool,
    },
    Skipped,
    Failed {
        provider_id: String,
        provider_type: String,
        key_id: String,
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
    let refresh_cutoff_unix_secs = now_unix_secs().saturating_add(config.lookahead_seconds);
    let mut candidates = Vec::<OAuthTokenRefreshCandidate>::new();

    'providers: for provider in providers {
        let provider_keys = keys_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let provider_endpoints = endpoints_by_provider
            .get(provider.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
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
                provider_type: provider.provider_type.clone(),
                key_id: key.id.clone(),
                key: key.clone(),
                transport,
            });
            if candidates.len() >= config.max_per_run {
                break 'providers;
            }
        }
    }

    let outcomes = stream::iter(candidates.into_iter().map(|candidate| async move {
        let resolution = state
            .resolve_local_oauth_request_auth_for_auto_refresh(&candidate.transport)
            .await;
        match resolution {
            Ok(Some(_auth)) => {
                match provider_key_credentials_changed(state, &candidate.key).await {
                    Ok(refreshed) => OAuthTokenRefreshCandidateOutcome::Resolved { refreshed },
                    Err(err) => OAuthTokenRefreshCandidateOutcome::Failed {
                        provider_id: candidate.provider_id,
                        provider_type: candidate.provider_type,
                        key_id: candidate.key_id,
                        error: format!("{err:?}"),
                    },
                }
            }
            Ok(None) => OAuthTokenRefreshCandidateOutcome::Skipped,
            Err(err) => OAuthTokenRefreshCandidateOutcome::Failed {
                provider_id: candidate.provider_id,
                provider_type: candidate.provider_type,
                key_id: candidate.key_id,
                error: format!("{err:?}"),
            },
        }
    }))
    .buffer_unordered(config.concurrency)
    .collect::<Vec<_>>()
    .await;

    for outcome in outcomes {
        match outcome {
            OAuthTokenRefreshCandidateOutcome::Resolved { refreshed } => {
                summary.resolved = summary.resolved.saturating_add(1);
                if refreshed {
                    summary.refreshed = summary.refreshed.saturating_add(1);
                }
            }
            OAuthTokenRefreshCandidateOutcome::Skipped => {
                summary.skipped = summary.skipped.saturating_add(1);
            }
            OAuthTokenRefreshCandidateOutcome::Failed {
                provider_id,
                provider_type,
                key_id,
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
                append_event_with_logging(
                    state,
                    &oauth_token_refresh_run_id(state),
                    "oauth_refresh_failed",
                    "oauth token refresh failed",
                    Some(serde_json::json!({
                        "provider_id": provider_id,
                        "provider_type": provider_type,
                        "key_id": key_id,
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
