use super::{
    admin_pool_dashboard_path, build_admin_pool_error_response, pool_payloads,
    AdminPoolDashboardPath, ADMIN_POOL_PROVIDER_CATALOG_READER_UNAVAILABLE_DETAIL,
    ADMIN_POOL_USAGE_READER_UNAVAILABLE_DETAIL,
};
use crate::handlers::admin::request::{AdminAppState, AdminRequestContext};
use crate::handlers::admin::shared::provider_key_status_snapshot_payload;
use crate::GatewayError;
use aether_admin::observability::stats::{
    parse_tz_offset_minutes, user_today, AdminStatsTimeRange,
};
use aether_data_contracts::repository::{
    provider_catalog::StoredProviderCatalogKey,
    quota::{
        ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery,
        ProviderKeyQuotaWindowObservation,
    },
    usage::{
        ProviderApiKeyWindowUsageRequest, StoredProviderApiKeyWindowUsageSummary,
        UsageProviderPerformanceQuery, UsageTimeSeriesGranularity, UsageTimeSeriesQuery,
    },
};
use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, NaiveDate};
use serde_json::{json, Value};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

const DEFAULT_PAGE_SIZE: usize = 25;
const MAX_PAGE_SIZE: usize = 100;
const FORECAST_LOOKBACK_SECS: u64 = 6 * 60 * 60;
const QUOTA_HISTORY_LOOKBACK_SECS: u64 = 400 * 24 * 60 * 60;
const MAX_INFERRED_QUOTA_CYCLES_PER_WINDOW: usize = 120;
const QUOTA_STALE_AFTER_SECS: u64 = 15 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DashboardGranularity {
    Hour,
    Day,
}

impl DashboardGranularity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Day => "day",
        }
    }

    fn usage(self) -> UsageTimeSeriesGranularity {
        match self {
            Self::Hour => UsageTimeSeriesGranularity::Hour,
            Self::Day => UsageTimeSeriesGranularity::Day,
        }
    }
}

#[derive(Clone, Debug)]
struct DashboardRange {
    key: String,
    label: String,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    start_unix_secs: u64,
    end_unix_secs: u64,
    previous: Option<(u64, u64)>,
    granularity: DashboardGranularity,
}

#[derive(Clone, Debug)]
struct DashboardQuery {
    range: String,
    start_date: Option<String>,
    end_date: Option<String>,
    start_unix_secs: Option<u64>,
    end_unix_secs: Option<u64>,
    tz_offset_minutes: i32,
    granularity: String,
    page: usize,
    page_size: usize,
    search: Option<String>,
    usage: String,
    active: String,
    risk: String,
    freshness: String,
    result: String,
    model: Option<String>,
    sort_by: String,
    sort_order: String,
}

#[derive(Clone, Debug)]
struct DashboardAccount {
    key_id: String,
    key_name: String,
    auth_type: String,
    is_active: bool,
    status: String,
    request_count: u64,
    successful_request_count: u64,
    failed_request_count: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    cache_hit_request_count: u64,
    total_cost_usd: f64,
    actual_total_cost_usd: f64,
    avg_first_byte_time_ms: Option<f64>,
    p95_first_byte_time_ms: Option<u64>,
    avg_response_time_ms: Option<f64>,
    p95_response_time_ms: Option<u64>,
    last_used_at_unix_secs: Option<u64>,
    quota: Value,
    quota_risk: String,
    quota_freshness: String,
    minimum_remaining_percent: Option<f64>,
    maximum_burn_rate: Option<f64>,
    earliest_exhaustion_unix_secs: Option<u64>,
    model_request_counts: BTreeMap<String, u64>,
    error_request_counts: BTreeMap<String, u64>,
}

impl DashboardAccount {
    fn success_rate(&self) -> Option<f64> {
        (self.request_count > 0)
            .then(|| self.successful_request_count as f64 * 100.0 / self.request_count as f64)
    }

    fn cache_hit_rate(&self) -> Option<f64> {
        (self.request_count > 0)
            .then(|| self.cache_hit_request_count as f64 * 100.0 / self.request_count as f64)
    }

    fn to_json(&self) -> Value {
        json!({
            "key_id": self.key_id,
            "key_name": self.key_name,
            "auth_type": self.auth_type,
            "is_active": self.is_active,
            "status": self.status,
            "request_count": self.request_count,
            "successful_request_count": self.successful_request_count,
            "failed_request_count": self.failed_request_count,
            "success_rate": self.success_rate(),
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "cache_creation_input_tokens": self.cache_creation_tokens,
            "cache_read_input_tokens": self.cache_read_tokens,
            "total_tokens": self.total_tokens,
            "cache_hit_request_count": self.cache_hit_request_count,
            "cache_hit_rate": self.cache_hit_rate(),
            "total_cost_usd": format_cost(self.total_cost_usd),
            "actual_total_cost_usd": format_cost(self.actual_total_cost_usd),
            "avg_first_byte_time_ms": self.avg_first_byte_time_ms,
            "p95_first_byte_time_ms": self.p95_first_byte_time_ms,
            "avg_response_time_ms": self.avg_response_time_ms,
            "p95_response_time_ms": self.p95_response_time_ms,
            "last_used_at_unix_secs": self.last_used_at_unix_secs,
            "quota": self.quota,
            "quota_risk": self.quota_risk,
            "quota_freshness": self.quota_freshness,
            "minimum_remaining_percent": self.minimum_remaining_percent,
            "maximum_burn_rate_percent_per_hour": self.maximum_burn_rate,
            "earliest_exhaustion_unix_secs": self.earliest_exhaustion_unix_secs,
        })
    }
}

pub(super) async fn build_admin_pool_consumption_dashboard_response(
    state: &AdminAppState<'_>,
    request_context: &AdminRequestContext<'_>,
) -> Result<Response<Body>, GatewayError> {
    let query_started = std::time::Instant::now();
    if !state.has_provider_catalog_data_reader() {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::SERVICE_UNAVAILABLE,
            ADMIN_POOL_PROVIDER_CATALOG_READER_UNAVAILABLE_DETAIL,
        ));
    }
    if !state.has_usage_data_reader() {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::SERVICE_UNAVAILABLE,
            ADMIN_POOL_USAGE_READER_UNAVAILABLE_DETAIL,
        ));
    }

    let path = match admin_pool_dashboard_path(request_context.path()) {
        Some(path) => path,
        None => {
            return Ok(build_admin_pool_error_response(
                http::StatusCode::BAD_REQUEST,
                "consumption dashboard path 无效",
            ));
        }
    };
    let query = match parse_dashboard_query(request_context.query_string()) {
        Ok(query) => query,
        Err(detail) => {
            return Ok(build_admin_pool_error_response(
                http::StatusCode::BAD_REQUEST,
                detail,
            ));
        }
    };
    let range = match resolve_dashboard_range(&query) {
        Ok(range) => range,
        Err(detail) => {
            return Ok(build_admin_pool_error_response(
                http::StatusCode::BAD_REQUEST,
                detail,
            ));
        }
    };

    let result = match path {
        AdminPoolDashboardPath::Overview { provider_id } => {
            build_dashboard_overview(state, &provider_id, &query, &range).await
        }
        AdminPoolDashboardPath::Account {
            provider_id,
            key_id,
        } => build_dashboard_account_detail(state, &provider_id, &key_id, &query, &range).await,
    };
    crate::data::state::observe_pool_consumption_dashboard_query(query_started.elapsed());
    result
}

async fn build_dashboard_overview(
    state: &AdminAppState<'_>,
    provider_id: &str,
    query: &DashboardQuery,
    range: &DashboardRange,
) -> Result<Response<Body>, GatewayError> {
    let Some(provider) = state
        .read_provider_catalog_providers_by_ids(&[provider_id.to_string()])
        .await?
        .into_iter()
        .next()
    else {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::NOT_FOUND,
            format!("Provider {provider_id} 不存在"),
        ));
    };
    let mut keys = state
        .list_provider_catalog_keys_by_provider_ids(&[provider_id.to_string()])
        .await?;
    keys.sort_by(|left, right| {
        left.internal_priority
            .cmp(&right.internal_priority)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });

    let current = summarize_key_range(
        state,
        &keys,
        "current",
        range.start_unix_secs,
        range.end_unix_secs,
        query.model.as_deref(),
    )
    .await?;
    let previous = if let Some((start, end)) = range.previous {
        summarize_key_range(state, &keys, "previous", start, end, query.model.as_deref()).await?
    } else {
        BTreeMap::new()
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let observations = state
        .app()
        .data
        .list_provider_key_quota_observations(&ProviderKeyQuotaObservationQuery {
            provider_id: provider_id.to_string(),
            provider_api_key_id: None,
            observed_from_unix_secs: Some(now.saturating_sub(FORECAST_LOOKBACK_SECS)),
            observed_until_unix_secs: Some(now.saturating_add(1)),
            limit: Some(keys.len().saturating_mul(80).clamp(80, 25_000)),
        })
        .await
        .map_err(|error| GatewayError::Internal(format!("quota history read failed: {error}")))?;
    let histories = group_observations_by_key(observations);

    let mut accounts = keys
        .iter()
        .map(|key| {
            let summary = current.get(&key.id).cloned().unwrap_or_default();
            build_dashboard_account(
                key,
                &provider.provider_type,
                summary,
                histories
                    .get(&key.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                now,
            )
        })
        .filter(|account| account_matches_query(account, query))
        .collect::<Vec<_>>();

    sort_accounts(&mut accounts, &query.sort_by, &query.sort_order);
    let total = accounts.len();
    let summary = build_accounts_summary(&accounts);
    let previous_accounts = keys
        .iter()
        .map(|key| {
            build_dashboard_account(
                key,
                &provider.provider_type,
                previous.get(&key.id).cloned().unwrap_or_default(),
                histories
                    .get(&key.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                now,
            )
        })
        .filter(|account| account_matches_query(account, query))
        .collect::<Vec<_>>();
    let previous_summary = range
        .previous
        .map(|_| build_accounts_summary(&previous_accounts));

    let offset = query.page.saturating_sub(1).saturating_mul(query.page_size);
    let account_page_items = accounts
        .iter()
        .skip(offset)
        .take(query.page_size)
        .collect::<Vec<_>>();
    let quota_window_usage = summarize_account_quota_windows(state, &account_page_items).await?;
    let account_page = account_page_items
        .into_iter()
        .map(|account| account_to_json_with_quota_usage(account, &quota_window_usage))
        .collect::<Vec<_>>();
    let burning_band = build_burning_band(&accounts);
    let selected_provider_api_key_ids = accounts
        .iter()
        .map(|account| account.key_id.clone())
        .collect::<Vec<_>>();

    let timeline = state
        .summarize_usage_time_series(&UsageTimeSeriesQuery {
            created_from_unix_secs: range.start_unix_secs,
            created_until_unix_secs: range.end_unix_secs,
            granularity: range.granularity.usage(),
            tz_offset_minutes: query.tz_offset_minutes,
            user_id: None,
            provider_name: None,
            provider_id: Some(provider.id.clone()),
            provider_api_key_ids: Some(selected_provider_api_key_ids.clone()),
            model: query.model.clone(),
        })
        .await?
        .into_iter()
        .map(|item| {
            json!({
                "bucket": item.bucket_key,
                "request_count": item.total_requests,
                "input_tokens": item.input_tokens,
                "output_tokens": item.output_tokens,
                "cache_creation_tokens": item.cache_creation_tokens,
                "cache_read_tokens": item.cache_read_tokens,
                "total_cost_usd": format_cost(item.total_cost_usd),
                "avg_response_time_ms": (item.total_requests > 0).then(|| item.total_response_time_ms / item.total_requests as f64),
            })
        })
        .collect::<Vec<_>>();
    let models = aggregate_account_dimensions(&accounts, |account| &account.model_request_counts)
        .into_iter()
        .map(|(model, request_count)| {
            json!({
                "model": model,
                "request_count": request_count,
                "total_tokens": 0,
                "total_cost_usd": format_cost(0.0),
                "actual_total_cost_usd": format_cost(0.0),
            })
        })
        .collect::<Vec<_>>();
    let errors = aggregate_account_dimensions(&accounts, |account| &account.error_request_counts)
        .into_iter()
        .map(|(error_category, count)| json!({ "error_category": error_category, "count": count }))
        .collect::<Vec<_>>();
    let performance = state
        .summarize_usage_provider_performance(&UsageProviderPerformanceQuery {
            created_from_unix_secs: range.start_unix_secs,
            created_until_unix_secs: range.end_unix_secs,
            granularity: range.granularity.usage(),
            tz_offset_minutes: query.tz_offset_minutes,
            limit: 1,
            provider_id: Some(provider.id.clone()),
            provider_api_key_ids: Some(selected_provider_api_key_ids),
            model: query.model.clone(),
            api_format: None,
            endpoint_kind: None,
            is_stream: None,
            has_format_conversion: None,
            slow_threshold_ms: 10_000,
            include_timeline: false,
        })
        .await?;

    Ok(Json(json!({
        "provider_id": provider.id,
        "provider_name": provider.name,
        "provider_type": provider.provider_type,
        "range": {
            "key": range.key,
            "label": range.label,
            "start_date": range.start_date.map(|value| value.to_string()),
            "end_date": range.end_date.map(|value| value.to_string()),
            "start_unix_secs": range.start_unix_secs,
            "end_unix_secs": range.end_unix_secs,
            "granularity": range.granularity.as_str(),
            "tz_offset_minutes": query.tz_offset_minutes,
        },
        "summary": summary,
        "previous_summary": previous_summary,
        "burning_band": burning_band,
        "charts": {
            "timeline": timeline,
            "models": models,
            "errors": errors,
            "performance": performance,
        },
        "accounts": account_page,
        "pagination": {
            "page": query.page,
            "page_size": query.page_size,
            "total": total,
            "total_pages": total.div_ceil(query.page_size),
        },
        "filters": {
            "search": query.search,
            "usage": query.usage,
            "active": query.active,
            "risk": query.risk,
            "freshness": query.freshness,
            "result": query.result,
            "model": query.model,
            "sort_by": query.sort_by,
            "sort_order": query.sort_order,
        },
    }))
    .into_response())
}

async fn build_dashboard_account_detail(
    state: &AdminAppState<'_>,
    provider_id: &str,
    key_id: &str,
    query: &DashboardQuery,
    range: &DashboardRange,
) -> Result<Response<Body>, GatewayError> {
    let Some(provider) = state
        .read_provider_catalog_providers_by_ids(&[provider_id.to_string()])
        .await?
        .into_iter()
        .next()
    else {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::NOT_FOUND,
            format!("Provider {provider_id} 不存在"),
        ));
    };
    let Some(key) = state
        .list_provider_catalog_keys_by_ids(&[key_id.to_string()])
        .await?
        .into_iter()
        .find(|key| key.provider_id == provider_id)
    else {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::NOT_FOUND,
            format!("账号 {key_id} 不存在或不属于当前 Provider"),
        ));
    };
    let summaries = summarize_key_range(
        state,
        std::slice::from_ref(&key),
        "detail",
        range.start_unix_secs,
        range.end_unix_secs,
        query.model.as_deref(),
    )
    .await?;
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let mut observations = state
        .app()
        .data
        .list_provider_key_quota_observations(&ProviderKeyQuotaObservationQuery {
            provider_id: provider_id.to_string(),
            provider_api_key_id: Some(key_id.to_string()),
            observed_from_unix_secs: Some(now.saturating_sub(QUOTA_HISTORY_LOOKBACK_SECS)),
            observed_until_unix_secs: Some(now.saturating_add(1)),
            limit: Some(50_000),
        })
        .await
        .map_err(|error| GatewayError::Internal(format!("quota history read failed: {error}")))?;
    // The current status snapshot is the freshest quota data available for an account,
    // but it may not have been persisted inside the selected reporting range yet. Keep it
    // in the detail history so a newly opened account never renders an empty data panel.
    if let Some(current) = ProviderKeyQuotaObservation::from_status_snapshot(
        key.provider_id.clone(),
        key.id.clone(),
        key.name.clone(),
        provider.provider_type.clone(),
        &provider_key_status_snapshot_payload(&key, &provider.provider_type),
        key.updated_at_unix_secs.unwrap_or(now),
    ) {
        if !observations.iter().any(|item| {
            item.observed_at_unix_secs == current.observed_at_unix_secs
                && item.bucket_start_unix_secs == current.bucket_start_unix_secs
        }) {
            observations.push(current);
        }
    }
    let account = build_dashboard_account(
        &key,
        &provider.provider_type,
        summaries.get(key_id).cloned().unwrap_or_default(),
        &observations,
        now,
    );
    let timeline = state
        .summarize_usage_time_series(&UsageTimeSeriesQuery {
            created_from_unix_secs: range.start_unix_secs,
            created_until_unix_secs: range.end_unix_secs,
            granularity: range.granularity.usage(),
            tz_offset_minutes: query.tz_offset_minutes,
            user_id: None,
            provider_name: None,
            provider_id: Some(provider.id.clone()),
            provider_api_key_ids: Some(vec![key.id.clone()]),
            model: query.model.clone(),
        })
        .await?
        .into_iter()
        .map(|item| {
            json!({
                "bucket": item.bucket_key,
                "request_count": item.total_requests,
                "input_tokens": item.input_tokens,
                "output_tokens": item.output_tokens,
                "cache_creation_tokens": item.cache_creation_tokens,
                "cache_read_tokens": item.cache_read_tokens,
                "total_tokens": item.input_tokens
                    .saturating_add(item.output_tokens)
                    .saturating_add(item.cache_creation_tokens)
                    .saturating_add(item.cache_read_tokens),
                "total_cost_usd": format_cost(item.total_cost_usd),
            })
        })
        .collect::<Vec<_>>();
    append_inferred_quota_cycles(&mut observations, now);
    let history = aggregate_quota_cycle_history(observations)
        .into_iter()
        .map(|observation| observation_to_json(&observation, None, now))
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "provider_id": provider.id,
        "provider_name": provider.name,
        "provider_type": provider.provider_type,
        "range": {
            "key": range.key,
            "start_unix_secs": range.start_unix_secs,
            "end_unix_secs": range.end_unix_secs,
            "granularity": range.granularity.as_str(),
            "quota_history_granularity": "cycle",
        },
        "account": account.to_json(),
        "charts": {
            "timeline": timeline,
        },
        "quota_history": history,
        "model_distribution": account.model_request_counts.iter().map(|(model, count)| json!({ "model": model, "request_count": count })).collect::<Vec<_>>(),
        "error_distribution": account.error_request_counts.iter().map(|(category, count)| json!({ "error_category": category, "count": count })).collect::<Vec<_>>(),
        "performance": {
            "avg_first_byte_time_ms": account.avg_first_byte_time_ms,
            "p95_first_byte_time_ms": account.p95_first_byte_time_ms,
            "avg_response_time_ms": account.avg_response_time_ms,
            "p95_response_time_ms": account.p95_response_time_ms,
        },
    }))
    .into_response())
}

async fn summarize_key_range(
    state: &AdminAppState<'_>,
    keys: &[StoredProviderCatalogKey],
    code: &str,
    start_unix_secs: u64,
    end_unix_secs: u64,
    model: Option<&str>,
) -> Result<BTreeMap<String, StoredProviderApiKeyWindowUsageSummary>, GatewayError> {
    if keys.is_empty() {
        return Ok(BTreeMap::new());
    }
    let requests = keys
        .iter()
        .map(|key| ProviderApiKeyWindowUsageRequest {
            provider_api_key_id: key.id.clone(),
            window_code: code.to_string(),
            start_unix_secs,
            end_unix_secs,
            model: model.map(ToOwned::to_owned),
        })
        .collect::<Vec<_>>();
    Ok(state
        .app()
        .summarize_usage_by_provider_api_key_windows(&requests)
        .await?
        .into_iter()
        .map(|item| (item.provider_api_key_id.clone(), item))
        .collect())
}

async fn summarize_account_quota_windows(
    state: &AdminAppState<'_>,
    accounts: &[&DashboardAccount],
) -> Result<BTreeMap<(String, String), StoredProviderApiKeyWindowUsageSummary>, GatewayError> {
    let mut requests = Vec::new();
    for account in accounts {
        let Some(windows) = account.quota.get("windows").and_then(Value::as_array) else {
            continue;
        };
        for window in windows {
            let Some(window_identity) = window.get("window_identity").and_then(Value::as_str)
            else {
                continue;
            };
            let Some(end_unix_secs) = window.get("reset_at_unix_secs").and_then(Value::as_u64)
            else {
                continue;
            };
            let Some(window_minutes) = window.get("window_minutes").and_then(Value::as_u64) else {
                continue;
            };
            if window_minutes == 0 {
                continue;
            }
            let start_unix_secs = end_unix_secs.saturating_sub(window_minutes.saturating_mul(60));
            if start_unix_secs >= end_unix_secs {
                continue;
            }
            requests.push(ProviderApiKeyWindowUsageRequest {
                provider_api_key_id: account.key_id.clone(),
                window_code: window_identity.to_string(),
                start_unix_secs,
                end_unix_secs,
                model: window
                    .get("model")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
        }
    }
    if requests.is_empty() {
        return Ok(BTreeMap::new());
    }
    Ok(state
        .app()
        .summarize_usage_by_provider_api_key_windows(&requests)
        .await?
        .into_iter()
        .map(|summary| {
            (
                (
                    summary.provider_api_key_id.clone(),
                    summary.window_code.clone(),
                ),
                summary,
            )
        })
        .collect())
}

fn account_to_json_with_quota_usage(
    account: &DashboardAccount,
    usage: &BTreeMap<(String, String), StoredProviderApiKeyWindowUsageSummary>,
) -> Value {
    let mut payload = account.to_json();
    let Some(windows) = payload
        .get_mut("quota")
        .and_then(|quota| quota.get_mut("windows"))
        .and_then(Value::as_array_mut)
    else {
        return payload;
    };
    for window in windows {
        let Some(window_identity) = window
            .get("window_identity")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        let Some(summary) = usage.get(&(account.key_id.clone(), window_identity)) else {
            continue;
        };
        let Some(window) = window.as_object_mut() else {
            continue;
        };
        window.insert(
            "local_request_count".to_string(),
            json!(summary.request_count),
        );
        window.insert(
            "local_total_tokens".to_string(),
            json!(summary.total_tokens),
        );
        window.insert(
            "local_cost_usd".to_string(),
            json!(format_cost(summary.total_cost_usd)),
        );
    }
    payload
}

fn build_dashboard_account(
    key: &StoredProviderCatalogKey,
    provider_type: &str,
    usage: StoredProviderApiKeyWindowUsageSummary,
    history: &[ProviderKeyQuotaObservation],
    now: u64,
) -> DashboardAccount {
    let normalized_status_snapshot = provider_key_status_snapshot_payload(key, provider_type);
    let current = ProviderKeyQuotaObservation::from_status_snapshot(
        key.provider_id.clone(),
        key.id.clone(),
        key.name.clone(),
        provider_type.to_string(),
        &normalized_status_snapshot,
        key.updated_at_unix_secs.unwrap_or(now),
    );
    let mut combined_history = history.to_vec();
    if let Some(current) = current.clone() {
        if !combined_history.iter().any(|item| {
            item.bucket_start_unix_secs == current.bucket_start_unix_secs
                && item.observed_at_unix_secs >= current.observed_at_unix_secs
        }) {
            combined_history.push(current);
        }
    }
    combined_history.sort_by_key(|item| item.observed_at_unix_secs);
    let quota = current
        .as_ref()
        .map(|current| observation_to_json(current, Some(&combined_history), now))
        .unwrap_or_else(|| {
            json!({
                "supported": false,
                "freshness": "unknown",
                "risk": "unknown",
                "message": "不支持或尚无额度快照",
                "windows": [],
                "legacy_text": pool_payloads::admin_pool_account_quota_from_key(key, provider_type),
            })
        });
    let quota_risk = quota
        .get("risk")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let quota_freshness = quota
        .get("freshness")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let minimum_remaining_percent = quota
        .get("minimum_remaining_percent")
        .and_then(Value::as_f64);
    let maximum_burn_rate = quota
        .get("maximum_burn_rate_percent_per_hour")
        .and_then(Value::as_f64);
    let earliest_exhaustion_unix_secs = quota
        .get("earliest_exhaustion_unix_secs")
        .and_then(Value::as_u64);
    let status = normalized_status_snapshot
        .get("account")
        .and_then(|account| account.get("code"))
        .and_then(Value::as_str)
        .unwrap_or(if key.is_active {
            "available"
        } else {
            "inactive"
        })
        .to_string();

    DashboardAccount {
        key_id: key.id.clone(),
        key_name: key.name.clone(),
        auth_type: key.auth_type.clone(),
        is_active: key.is_active,
        status,
        request_count: usage.request_count,
        successful_request_count: usage.successful_request_count,
        failed_request_count: usage.failed_request_count,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        total_tokens: usage.total_tokens,
        cache_hit_request_count: usage.cache_hit_request_count,
        total_cost_usd: usage.total_cost_usd,
        actual_total_cost_usd: usage.actual_total_cost_usd,
        avg_first_byte_time_ms: usage.avg_first_byte_time_ms,
        p95_first_byte_time_ms: usage.p95_first_byte_time_ms,
        avg_response_time_ms: usage.avg_response_time_ms,
        p95_response_time_ms: usage.p95_response_time_ms,
        last_used_at_unix_secs: usage.last_used_at_unix_secs,
        quota,
        quota_risk,
        quota_freshness,
        minimum_remaining_percent,
        maximum_burn_rate,
        earliest_exhaustion_unix_secs,
        model_request_counts: usage.model_request_counts,
        error_request_counts: usage.error_request_counts,
    }
}

fn observation_to_json(
    observation: &ProviderKeyQuotaObservation,
    history: Option<&[ProviderKeyQuotaObservation]>,
    now: u64,
) -> Value {
    let freshness =
        if now.saturating_sub(observation.observed_at_unix_secs) > QUOTA_STALE_AFTER_SECS {
            "stale"
        } else {
            "fresh"
        };
    let has_percentage_window = observation
        .windows
        .iter()
        .any(|window| window.used_percent.is_some() || window.remaining_percent.is_some());
    let mut risk = if has_percentage_window || observation.credits_unlimited == Some(true) {
        "healthy".to_string()
    } else {
        "unknown".to_string()
    };
    let mut minimum_remaining_percent: Option<f64> = None;
    let mut maximum_burn_rate: Option<f64> = None;
    let mut earliest_exhaustion: Option<u64> = None;
    let windows = observation
        .windows
        .iter()
        .map(|window| {
            let mut forecast =
                forecast_window(observation, window, history.unwrap_or_default(), now);
            if history.is_none() {
                if let Some(ideal_used_percent) =
                    ideal_used_percent(window, observation.observed_at_unix_secs)
                {
                    if let Some(forecast) = forecast.as_object_mut() {
                        forecast
                            .insert("ideal_used_percent".to_string(), json!(ideal_used_percent));
                    }
                }
            }
            let window_risk = forecast
                .get("risk")
                .and_then(Value::as_str)
                .unwrap_or("healthy");
            if risk_rank(window_risk) > risk_rank(&risk) {
                risk = window_risk.to_string();
            }
            if let Some(remaining) = window.remaining_percent {
                minimum_remaining_percent = Some(
                    minimum_remaining_percent
                        .map(|current| current.min(remaining))
                        .unwrap_or(remaining),
                );
            }
            if let Some(rate) = forecast
                .get("burn_rate_percent_per_hour")
                .and_then(Value::as_f64)
            {
                maximum_burn_rate = Some(
                    maximum_burn_rate
                        .map(|current| current.max(rate))
                        .unwrap_or(rate),
                );
            }
            if let Some(exhaustion) = forecast
                .get("estimated_exhaustion_unix_secs")
                .and_then(Value::as_u64)
            {
                earliest_exhaustion = Some(
                    earliest_exhaustion
                        .map(|current| current.min(exhaustion))
                        .unwrap_or(exhaustion),
                );
            }
            json!({
                "window_identity": window.window_identity,
                "code": window.code,
                "label": window.label,
                "scope": window.scope,
                "model": window.model,
                "unit": window.unit,
                "used_percent": window.used_percent,
                "remaining_percent": window.remaining_percent,
                "used_value": window.used_value,
                "remaining_value": window.remaining_value,
                "limit_value": window.limit_value,
                "reset_at_unix_secs": window.reset_at_unix_secs,
                "window_minutes": window.window_minutes,
                "exhausted": window.exhausted,
                "local_request_count": window.local_request_count,
                "local_total_tokens": window.local_total_tokens,
                "local_cost_usd": format_cost(window.local_cost_usd),
                "forecast": forecast,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "supported": true,
        "observed_at_unix_secs": observation.observed_at_unix_secs,
        "source": observation.source,
        "plan_type": observation.plan_type,
        "status_code": observation.status_code,
        "status_label": observation.status_label,
        "freshness": freshness,
        "risk": risk,
        "credits_balance": observation.credits_balance.map(format_decimal),
        "credits_unlimited": observation.credits_unlimited,
        "reset_credits_count": observation.reset_credits_count,
        "minimum_remaining_percent": minimum_remaining_percent,
        "maximum_burn_rate_percent_per_hour": maximum_burn_rate,
        "earliest_exhaustion_unix_secs": earliest_exhaustion,
        "windows": windows,
    })
}

fn forecast_window(
    current_observation: &ProviderKeyQuotaObservation,
    current: &ProviderKeyQuotaWindowObservation,
    history: &[ProviderKeyQuotaObservation],
    now: u64,
) -> Value {
    let current_used = current.used_percent;
    let current_remaining = current
        .remaining_percent
        .or_else(|| current_used.map(|value| 100.0 - value));
    let ideal_used = ideal_used_percent(current, now);

    let mut samples = history
        .iter()
        .filter(|observation| {
            observation.observed_at_unix_secs >= now.saturating_sub(FORECAST_LOOKBACK_SECS)
        })
        .filter_map(|observation| {
            let window = observation.windows.iter().find(|candidate| {
                candidate.code == current.code
                    && candidate.scope == current.scope
                    && candidate.model == current.model
                    && candidate.reset_at_unix_secs == current.reset_at_unix_secs
                    && candidate.window_minutes == current.window_minutes
            })?;
            Some((observation.observed_at_unix_secs, window.used_percent?))
        })
        .collect::<Vec<_>>();
    if current_used.is_some()
        && !samples
            .iter()
            .any(|(observed_at, _)| *observed_at == current_observation.observed_at_unix_secs)
    {
        samples.push((
            current_observation.observed_at_unix_secs,
            current_used.unwrap_or_default(),
        ));
    }
    samples.sort_by_key(|sample| sample.0);
    samples.dedup_by_key(|sample| sample.0);
    let span_secs = samples
        .first()
        .zip(samples.last())
        .map(|(first, last)| last.0.saturating_sub(first.0))
        .unwrap_or(0);
    let confidence = if samples.len() >= 6 && span_secs >= 30 * 60 {
        "high"
    } else if samples.len() >= 2 && span_secs >= 15 * 60 {
        "medium"
    } else {
        "low"
    };
    let burn_rate = if confidence == "low" {
        None
    } else {
        ewma_burn_rate(&samples)
    };
    let estimated_exhaustion = burn_rate.and_then(|rate| {
        let remaining = current_remaining?;
        (rate > 0.0).then(|| now.saturating_add((remaining / rate * 3600.0) as u64))
    });
    let exhausts_before_reset = estimated_exhaustion
        .zip(current.reset_at_unix_secs)
        .map(|(exhaustion, reset)| exhaustion < reset)
        .unwrap_or(false);
    let pace_delta = current_used
        .zip(ideal_used)
        .map(|(actual, ideal)| actual - ideal);
    let risk = if current.exhausted || current_remaining.is_some_and(|value| value <= 0.0) {
        "exhausted"
    } else if current_remaining.is_none() {
        "unknown"
    } else if current_remaining.is_some_and(|value| value <= 10.0) || exhausts_before_reset {
        "critical"
    } else if current_remaining.is_some_and(|value| value <= 30.0)
        || pace_delta.is_some_and(|value| value > 5.0)
    {
        "warning"
    } else {
        "healthy"
    };

    json!({
        "confidence": confidence,
        "sample_count": samples.len(),
        "sample_span_seconds": span_secs,
        "actual_used_percent": current_used,
        "ideal_used_percent": ideal_used,
        "pace_delta_percent": pace_delta,
        "burn_rate_percent_per_hour": burn_rate,
        "estimated_exhaustion_unix_secs": estimated_exhaustion,
        "exhausts_before_reset": exhausts_before_reset,
        "risk": risk,
        "message": if confidence == "low" { Some("数据不足") } else { None },
    })
}

fn ideal_used_percent(
    window: &ProviderKeyQuotaWindowObservation,
    at_unix_secs: u64,
) -> Option<f64> {
    match (window.reset_at_unix_secs, window.window_minutes) {
        (Some(reset_at), Some(window_minutes)) if window_minutes > 0 => {
            let start = reset_at.saturating_sub(window_minutes.saturating_mul(60));
            let elapsed = at_unix_secs.saturating_sub(start) as f64;
            let duration = window_minutes.saturating_mul(60) as f64;
            Some((elapsed / duration * 100.0).clamp(0.0, 100.0))
        }
        _ => None,
    }
}

fn ewma_burn_rate(samples: &[(u64, f64)]) -> Option<f64> {
    let mut ewma = None;
    for pair in samples.windows(2) {
        let elapsed = pair[1].0.saturating_sub(pair[0].0);
        if elapsed == 0 {
            continue;
        }
        let rate = ((pair[1].1 - pair[0].1).max(0.0)) * 3600.0 / elapsed as f64;
        let alpha = 1.0 - (-(elapsed as f64) / 1800.0).exp();
        ewma = Some(match ewma {
            Some(previous) => alpha * rate + (1.0 - alpha) * previous,
            None => rate,
        });
    }
    ewma.filter(|value| value.is_finite())
}

fn group_observations_by_key(
    observations: Vec<ProviderKeyQuotaObservation>,
) -> BTreeMap<String, Vec<ProviderKeyQuotaObservation>> {
    let mut grouped = BTreeMap::new();
    for observation in observations {
        grouped
            .entry(observation.provider_api_key_id.clone())
            .or_insert_with(Vec::new)
            .push(observation);
    }
    grouped
}

fn append_inferred_quota_cycles(observations: &mut Vec<ProviderKeyQuotaObservation>, now: u64) {
    let earliest_end = now.saturating_sub(QUOTA_HISTORY_LOOKBACK_SECS);
    let mut known_cycles = BTreeSet::new();
    let mut latest_windows = BTreeMap::new();

    for observation in observations.iter() {
        for window in &observation.windows {
            let (Some(reset_at), Some(window_minutes)) =
                (window.reset_at_unix_secs, window.window_minutes)
            else {
                continue;
            };
            if window_minutes == 0 {
                continue;
            }
            let schedule_key = (
                window.code.clone(),
                window.scope.clone(),
                window.model.clone(),
                window_minutes,
            );
            known_cycles.insert((schedule_key.clone(), reset_at));
            let replace = latest_windows
                .get(&schedule_key)
                .is_none_or(|(_, current_reset)| reset_at > *current_reset);
            if replace {
                latest_windows.insert(schedule_key, (observation.clone(), reset_at));
            }
        }
    }

    for (schedule_key, (seed, reset_at)) in latest_windows {
        let Some(window_secs) = schedule_key.3.checked_mul(60) else {
            continue;
        };
        let Some(seed_window) = seed.windows.iter().find(|window| {
            window.code == schedule_key.0
                && window.scope == schedule_key.1
                && window.model == schedule_key.2
                && window.window_minutes == Some(schedule_key.3)
                && window.reset_at_unix_secs == Some(reset_at)
        }) else {
            continue;
        };
        let mut inferred_reset_at = reset_at.saturating_sub(window_secs);
        for _ in 0..MAX_INFERRED_QUOTA_CYCLES_PER_WINDOW {
            if inferred_reset_at == 0 || inferred_reset_at < earliest_end {
                break;
            }
            if known_cycles.insert((schedule_key.clone(), inferred_reset_at)) {
                let mut observation = seed.clone();
                observation.source = "derived_usage_window".to_string();
                observation.freshness = Some("unknown".to_string());
                observation.observed_at_unix_secs = inferred_reset_at.saturating_sub(1);
                observation.bucket_start_unix_secs = observation.observed_at_unix_secs;
                observation.credits_balance = None;
                observation.credits_unlimited = None;
                observation.reset_credits_count = 0;

                let mut window = seed_window.clone();
                window.used_percent = None;
                window.remaining_percent = None;
                window.used_value = None;
                window.remaining_value = None;
                window.limit_value = None;
                window.reset_at_unix_secs = Some(inferred_reset_at);
                window.exhausted = false;
                window.local_request_count = 0;
                window.local_total_tokens = 0;
                window.local_cost_usd = 0.0;
                observation.windows = vec![window];
                observations.push(observation);
            }
            let next_reset_at = inferred_reset_at.saturating_sub(window_secs);
            if next_reset_at == inferred_reset_at {
                break;
            }
            inferred_reset_at = next_reset_at;
        }
    }
}

fn aggregate_quota_cycle_history(
    observations: Vec<ProviderKeyQuotaObservation>,
) -> Vec<ProviderKeyQuotaObservation> {
    let mut by_cycle = BTreeMap::new();
    for observation in observations {
        for window in &observation.windows {
            let cycle_key = (
                window.code.clone(),
                window.scope.clone(),
                window.model.clone(),
                window.reset_at_unix_secs,
                window.window_minutes,
            );
            if by_cycle
                .get(&cycle_key)
                .is_none_or(|current: &ProviderKeyQuotaObservation| {
                    current.observed_at_unix_secs < observation.observed_at_unix_secs
                })
            {
                let mut cycle_observation = observation.clone();
                cycle_observation.windows = vec![window.clone()];
                by_cycle.insert(cycle_key, cycle_observation);
            }
        }
    }
    let mut cycles = by_cycle.into_values().collect::<Vec<_>>();
    cycles.sort_by_key(|observation| std::cmp::Reverse(observation.observed_at_unix_secs));
    cycles
}

fn account_matches_query(account: &DashboardAccount, query: &DashboardQuery) -> bool {
    if let Some(search) = &query.search {
        let haystack = format!("{} {}", account.key_name, account.auth_type).to_ascii_lowercase();
        if !haystack.contains(search) {
            return false;
        }
    }
    if query.usage == "used" && account.request_count == 0 {
        return false;
    }
    if query.usage == "idle" && account.request_count > 0 {
        return false;
    }
    if query.active == "active" && !account.is_active {
        return false;
    }
    if query.active == "inactive" && account.is_active {
        return false;
    }
    if query.active == "blocked" && !account_status_is_blocked(&account.status) {
        return false;
    }
    if query.risk != "all" && account.quota_risk != query.risk {
        return false;
    }
    if query.freshness != "all" && account.quota_freshness != query.freshness {
        return false;
    }
    if query.result == "success" && account.successful_request_count == 0 {
        return false;
    }
    if query.result == "failed" && account.failed_request_count == 0 {
        return false;
    }
    if query
        .model
        .as_ref()
        .is_some_and(|model| !account.model_request_counts.contains_key(model))
    {
        return false;
    }
    true
}

fn account_status_is_blocked(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    [
        "blocked",
        "banned",
        "forbidden",
        "invalid",
        "quarantined",
        "exhausted",
        "cooldown",
        "rate_limited",
        "deactivated",
        "verification",
    ]
    .iter()
    .any(|value| status.contains(value))
}

fn aggregate_account_dimensions(
    accounts: &[DashboardAccount],
    select: impl Fn(&DashboardAccount) -> &BTreeMap<String, u64>,
) -> Vec<(String, u64)> {
    let mut values = BTreeMap::<String, u64>::new();
    for account in accounts {
        for (dimension, count) in select(account) {
            let value = values.entry(dimension.clone()).or_default();
            *value = value.saturating_add(*count);
        }
    }
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values
}

fn sort_accounts(accounts: &mut [DashboardAccount], sort_by: &str, sort_order: &str) {
    accounts.sort_by(|left, right| {
        let ordering = match sort_by {
            "actual_cost" => left
                .actual_total_cost_usd
                .total_cmp(&right.actual_total_cost_usd),
            "tokens" => left.total_tokens.cmp(&right.total_tokens),
            "requests" => left.request_count.cmp(&right.request_count),
            "success_rate" => option_f64_cmp(left.success_rate(), right.success_rate()),
            "cache_hit" => option_f64_cmp(left.cache_hit_rate(), right.cache_hit_rate()),
            "p95_ttft" => left
                .p95_first_byte_time_ms
                .cmp(&right.p95_first_byte_time_ms),
            "p95_latency" => left.p95_response_time_ms.cmp(&right.p95_response_time_ms),
            "quota" => option_f64_cmp(
                left.minimum_remaining_percent,
                right.minimum_remaining_percent,
            ),
            "burn_rate" => option_f64_cmp(left.maximum_burn_rate, right.maximum_burn_rate),
            "last_used" => left
                .last_used_at_unix_secs
                .cmp(&right.last_used_at_unix_secs),
            _ => left.total_cost_usd.total_cmp(&right.total_cost_usd),
        };
        let ordering = if sort_order == "asc" {
            ordering
        } else {
            ordering.reverse()
        };
        ordering.then_with(|| left.key_name.cmp(&right.key_name))
    });
}

fn option_f64_cmp(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn build_accounts_summary(accounts: &[DashboardAccount]) -> Value {
    let request_count = accounts.iter().map(|item| item.request_count).sum::<u64>();
    let successful_request_count = accounts
        .iter()
        .map(|item| item.successful_request_count)
        .sum::<u64>();
    let failed_request_count = accounts
        .iter()
        .map(|item| item.failed_request_count)
        .sum::<u64>();
    let cache_hit_request_count = accounts
        .iter()
        .map(|item| item.cache_hit_request_count)
        .sum::<u64>();
    let total_tokens = accounts.iter().map(|item| item.total_tokens).sum::<u64>();
    let total_cost = accounts.iter().map(|item| item.total_cost_usd).sum::<f64>();
    let actual_cost = accounts
        .iter()
        .map(|item| item.actual_total_cost_usd)
        .sum::<f64>();
    let p95_ttft = accounts
        .iter()
        .filter_map(|item| item.p95_first_byte_time_ms)
        .max();
    let p95_latency = accounts
        .iter()
        .filter_map(|item| item.p95_response_time_ms)
        .max();
    json!({
        "account_count": accounts.len(),
        "used_account_count": accounts.iter().filter(|item| item.request_count > 0).count(),
        "idle_account_count": accounts.iter().filter(|item| item.request_count == 0).count(),
        "request_count": request_count,
        "successful_request_count": successful_request_count,
        "failed_request_count": failed_request_count,
        "success_rate": (request_count > 0).then(|| successful_request_count as f64 * 100.0 / request_count as f64),
        "input_tokens": accounts.iter().map(|item| item.input_tokens).sum::<u64>(),
        "output_tokens": accounts.iter().map(|item| item.output_tokens).sum::<u64>(),
        "cache_creation_input_tokens": accounts.iter().map(|item| item.cache_creation_tokens).sum::<u64>(),
        "cache_read_input_tokens": accounts.iter().map(|item| item.cache_read_tokens).sum::<u64>(),
        "total_tokens": total_tokens,
        "cache_hit_request_count": cache_hit_request_count,
        "cache_hit_rate": (request_count > 0).then(|| cache_hit_request_count as f64 * 100.0 / request_count as f64),
        "total_cost_usd": format_cost(total_cost),
        "actual_total_cost_usd": format_cost(actual_cost),
        "p95_first_byte_time_ms": p95_ttft,
        "p95_response_time_ms": p95_latency,
    })
}

fn build_burning_band(accounts: &[DashboardAccount]) -> Value {
    let mut ordered = accounts.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        risk_rank(&right.quota_risk)
            .cmp(&risk_rank(&left.quota_risk))
            .then_with(|| {
                left.earliest_exhaustion_unix_secs
                    .cmp(&right.earliest_exhaustion_unix_secs)
            })
            .then_with(|| left.key_name.cmp(&right.key_name))
    });
    json!({
        "counts": {
            "healthy": accounts.iter().filter(|item| item.quota_risk == "healthy").count(),
            "warning": accounts.iter().filter(|item| item.quota_risk == "warning").count(),
            "critical": accounts.iter().filter(|item| item.quota_risk == "critical").count(),
            "exhausted": accounts.iter().filter(|item| item.quota_risk == "exhausted").count(),
            "unknown": accounts.iter().filter(|item| item.quota_risk == "unknown").count(),
            "stale": accounts.iter().filter(|item| item.quota_freshness == "stale").count(),
        },
        "accounts": ordered.into_iter().take(12).map(DashboardAccount::to_json).collect::<Vec<_>>(),
    })
}

fn risk_rank(value: &str) -> u8 {
    match value {
        "exhausted" => 4,
        "critical" => 3,
        "warning" => 2,
        "healthy" => 1,
        _ => 0,
    }
}

fn parse_dashboard_query(query: Option<&str>) -> Result<DashboardQuery, String> {
    let params = query
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let tz_offset_minutes = parse_tz_offset_minutes(query)?;
    let page = parse_usize_param(&params, "page", 1, 1, usize::MAX)?;
    let page_size = parse_usize_param(&params, "page_size", DEFAULT_PAGE_SIZE, 1, MAX_PAGE_SIZE)?;
    let start_unix_secs = parse_optional_u64_param(&params, "start_unix_secs")?;
    let end_unix_secs = parse_optional_u64_param(&params, "end_unix_secs")?;
    match (start_unix_secs, end_unix_secs) {
        (Some(start), Some(end)) if start >= end => {
            return Err("start_unix_secs 必须早于 end_unix_secs".to_string());
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err("start_unix_secs 和 end_unix_secs 必须同时提供".to_string());
        }
        _ => {}
    }
    let range = params
        .get("range")
        .cloned()
        .unwrap_or_else(|| "last7days".to_string());
    if !matches!(
        range.as_str(),
        "today" | "last3days" | "last7days" | "last30days" | "last90days" | "all" | "custom"
    ) {
        return Err("range 无效".to_string());
    }
    let granularity = params
        .get("granularity")
        .cloned()
        .unwrap_or_else(|| "auto".to_string());
    if !matches!(granularity.as_str(), "auto" | "hour" | "day") {
        return Err("granularity 必须是 auto、hour 或 day".to_string());
    }
    let usage = enum_param(&params, "usage", "all", &["all", "used", "idle"])?;
    let active = enum_param(
        &params,
        "active",
        "all",
        &["all", "active", "inactive", "blocked"],
    )?;
    let risk = enum_param(
        &params,
        "risk",
        "all",
        &[
            "all",
            "healthy",
            "warning",
            "critical",
            "exhausted",
            "unknown",
        ],
    )?;
    let freshness = enum_param(
        &params,
        "freshness",
        "all",
        &["all", "fresh", "stale", "unknown"],
    )?;
    let result = enum_param(&params, "result", "all", &["all", "success", "failed"])?;
    let sort_order = enum_param(&params, "sort_order", "desc", &["asc", "desc"])?;
    Ok(DashboardQuery {
        range,
        start_date: params.get("start_date").cloned(),
        end_date: params.get("end_date").cloned(),
        start_unix_secs,
        end_unix_secs,
        tz_offset_minutes,
        granularity,
        page,
        page_size,
        search: normalized_param(&params, "search").map(|value| value.to_ascii_lowercase()),
        usage,
        active,
        risk,
        freshness,
        result,
        model: normalized_param(&params, "model"),
        sort_by: params
            .get("sort_by")
            .cloned()
            .unwrap_or_else(|| "cost".to_string()),
        sort_order,
    })
}

fn resolve_dashboard_range(query: &DashboardQuery) -> Result<DashboardRange, String> {
    if let (Some(start_unix_secs), Some(end_unix_secs)) =
        (query.start_unix_secs, query.end_unix_secs)
    {
        let duration = end_unix_secs.saturating_sub(start_unix_secs);
        let granularity = match query.granularity.as_str() {
            "hour" => DashboardGranularity::Hour,
            "day" => DashboardGranularity::Day,
            _ if duration <= 7 * 24 * 60 * 60 => DashboardGranularity::Hour,
            _ => DashboardGranularity::Day,
        };
        return Ok(DashboardRange {
            key: "quota_window".to_string(),
            label: "额度窗口".to_string(),
            start_date: None,
            end_date: None,
            start_unix_secs,
            end_unix_secs,
            previous: None,
            granularity,
        });
    }

    let today = user_today(query.tz_offset_minutes);
    let (label, start_date, end_date) = match query.range.as_str() {
        "today" => ("今天", Some(today), Some(today)),
        "last3days" => (
            "近 3 天",
            today.checked_sub_signed(Duration::days(2)),
            Some(today),
        ),
        "last7days" => (
            "近 7 天",
            today.checked_sub_signed(Duration::days(6)),
            Some(today),
        ),
        "last30days" => (
            "近 30 天",
            today.checked_sub_signed(Duration::days(29)),
            Some(today),
        ),
        "last90days" => (
            "近 90 天",
            today.checked_sub_signed(Duration::days(89)),
            Some(today),
        ),
        "all" => ("全部", None, None),
        "custom" => {
            let start = parse_date(query.start_date.as_deref(), "start_date")?;
            let end = parse_date(query.end_date.as_deref(), "end_date")?;
            if start > end {
                return Err("start_date 不能晚于 end_date".to_string());
            }
            ("自定义", Some(start), Some(end))
        }
        _ => return Err("range 无效".to_string()),
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let (start_unix_secs, end_unix_secs) = match (start_date, end_date) {
        (Some(start_date), Some(end_date)) => AdminStatsTimeRange {
            start_date,
            end_date,
            tz_offset_minutes: query.tz_offset_minutes,
        }
        .to_unix_bounds()
        .ok_or_else(|| "日期范围无效".to_string())?,
        _ => (0, now.saturating_add(1)),
    };
    let duration = end_unix_secs.saturating_sub(start_unix_secs);
    let previous =
        (query.range != "all").then(|| (start_unix_secs.saturating_sub(duration), start_unix_secs));
    let granularity = match query.granularity.as_str() {
        "hour" => DashboardGranularity::Hour,
        "day" => DashboardGranularity::Day,
        _ if duration <= 7 * 24 * 60 * 60 => DashboardGranularity::Hour,
        _ => DashboardGranularity::Day,
    };
    Ok(DashboardRange {
        key: query.range.clone(),
        label: label.to_string(),
        start_date,
        end_date,
        start_unix_secs,
        end_unix_secs,
        previous,
        granularity,
    })
}

fn parse_date(value: Option<&str>, field: &str) -> Result<NaiveDate, String> {
    let value = value.ok_or_else(|| format!("custom range 缺少 {field}"))?;
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| format!("{field} 必须是 YYYY-MM-DD"))
}

fn parse_usize_param(
    params: &BTreeMap<String, String>,
    key: &str,
    default: usize,
    min: usize,
    max: usize,
) -> Result<usize, String> {
    let Some(value) = params.get(key) else {
        return Ok(default);
    };
    let value = value
        .parse::<usize>()
        .map_err(|_| format!("{key} 必须是整数"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{key} 必须在 {min} 到 {max} 之间"));
    }
    Ok(value)
}

fn parse_optional_u64_param(
    params: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<u64>, String> {
    params
        .get(key)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("{key} 必须是非负整数"))
        })
        .transpose()
}

fn enum_param(
    params: &BTreeMap<String, String>,
    key: &str,
    default: &str,
    allowed: &[&str],
) -> Result<String, String> {
    let value = params.get(key).map(String::as_str).unwrap_or(default);
    allowed
        .contains(&value)
        .then(|| value.to_string())
        .ok_or_else(|| format!("{key} 无效"))
}

fn normalized_param(params: &BTreeMap<String, String>, key: &str) -> Option<String> {
    params
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn format_cost(value: f64) -> String {
    format!(
        "{:.8}",
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    )
}

fn format_decimal(value: f64) -> String {
    let mut value = format!("{:.8}", if value.is_finite() { value } else { 0.0 });
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.push('0');
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quota_observation(
        observed_at_unix_secs: u64,
        reset_at_unix_secs: u64,
        used_percent: f64,
    ) -> ProviderKeyQuotaObservation {
        ProviderKeyQuotaObservation {
            provider_id: "provider".into(),
            provider_api_key_id: "key".into(),
            provider_api_key_name: "Key".into(),
            provider_type: "codex".into(),
            bucket_start_unix_secs: observed_at_unix_secs,
            observed_at_unix_secs,
            source: "test".into(),
            plan_type: None,
            status_code: None,
            status_label: None,
            freshness: None,
            credits_balance: None,
            credits_unlimited: None,
            reset_credits_count: 0,
            windows: vec![ProviderKeyQuotaWindowObservation {
                window_identity: "monthly|account||0".into(),
                code: "monthly".into(),
                label: "月".into(),
                scope: Some("account".into()),
                model: None,
                unit: Some("percent".into()),
                used_percent: Some(used_percent),
                remaining_percent: Some(100.0 - used_percent),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                reset_at_unix_secs: Some(reset_at_unix_secs),
                window_minutes: Some(60),
                exhausted: false,
                local_request_count: 0,
                local_total_tokens: 0,
                local_cost_usd: 0.0,
            }],
        }
    }

    #[test]
    fn quota_cycle_history_keeps_latest_snapshot_for_each_reset() {
        let cycles = aggregate_quota_cycle_history(vec![
            quota_observation(1_100, 2_000, 10.0),
            quota_observation(1_200, 2_000, 20.0),
            quota_observation(900, 1_000, 100.0),
        ]);

        assert_eq!(cycles.len(), 2);
        assert_eq!(cycles[0].observed_at_unix_secs, 1_200);
        assert_eq!(cycles[0].windows[0].reset_at_unix_secs, Some(2_000));
        assert_eq!(cycles[0].windows[0].used_percent, Some(20.0));
        assert_eq!(cycles[1].windows[0].reset_at_unix_secs, Some(1_000));
    }

    #[test]
    fn inferred_quota_cycles_preserve_real_snapshots_without_copying_quota_values() {
        let now = QUOTA_HISTORY_LOOKBACK_SECS + 20_000;
        let previous_reset = now - 60 * 60;
        let mut observations = vec![
            quota_observation(now - 100, now, 10.0),
            quota_observation(previous_reset - 100, previous_reset, 80.0),
        ];

        append_inferred_quota_cycles(&mut observations, now);
        let cycles = aggregate_quota_cycle_history(observations);

        assert_eq!(cycles.len(), MAX_INFERRED_QUOTA_CYCLES_PER_WINDOW + 1);
        let real_previous = cycles
            .iter()
            .find(|item| item.windows[0].reset_at_unix_secs == Some(previous_reset))
            .expect("real previous cycle should remain");
        assert_eq!(real_previous.source, "test");
        assert_eq!(real_previous.windows[0].used_percent, Some(80.0));

        let inferred = cycles
            .iter()
            .find(|item| item.windows[0].reset_at_unix_secs == Some(previous_reset - 60 * 60))
            .expect("older cycle should be inferred");
        assert_eq!(inferred.source, "derived_usage_window");
        assert_eq!(inferred.freshness.as_deref(), Some("unknown"));
        assert_eq!(inferred.windows[0].used_percent, None);
        assert_eq!(inferred.windows[0].remaining_percent, None);
        assert_eq!(inferred.windows[0].local_request_count, 0);
    }

    #[test]
    fn dashboard_range_prefers_explicit_unix_bounds() {
        let query = parse_dashboard_query(Some(
            "range=last7days&start_unix_secs=1000&end_unix_secs=4600",
        ))
        .expect("query should parse");

        let range = resolve_dashboard_range(&query).expect("range should resolve");

        assert_eq!(range.key, "quota_window");
        assert_eq!(range.start_unix_secs, 1_000);
        assert_eq!(range.end_unix_secs, 4_600);
        assert_eq!(range.granularity, DashboardGranularity::Hour);
        assert!(range.previous.is_none());
    }

    #[test]
    fn forecast_requires_enough_span_for_risk_prediction() {
        let current = ProviderKeyQuotaObservation {
            provider_id: "provider".into(),
            provider_api_key_id: "key".into(),
            provider_api_key_name: "Key".into(),
            provider_type: "codex".into(),
            bucket_start_unix_secs: 1_800,
            observed_at_unix_secs: 2_000,
            source: "test".into(),
            plan_type: None,
            status_code: None,
            status_label: None,
            freshness: None,
            credits_balance: None,
            credits_unlimited: None,
            reset_credits_count: 0,
            windows: vec![ProviderKeyQuotaWindowObservation {
                window_identity: "weekly|||0".into(),
                code: "weekly".into(),
                label: "周额度".into(),
                scope: None,
                model: None,
                unit: Some("percent".into()),
                used_percent: Some(50.0),
                remaining_percent: Some(50.0),
                used_value: None,
                remaining_value: None,
                limit_value: None,
                reset_at_unix_secs: Some(8_000),
                window_minutes: Some(120),
                exhausted: false,
                local_request_count: 0,
                local_total_tokens: 0,
                local_cost_usd: 0.0,
            }],
        };
        let forecast = forecast_window(&current, &current.windows[0], &[current.clone()], 2_000);
        assert_eq!(forecast["confidence"], "low");
        assert!(forecast["estimated_exhaustion_unix_secs"].is_null());
    }

    #[test]
    fn dashboard_range_uses_hour_granularity_within_seven_days() {
        let query = parse_dashboard_query(Some("range=last7days&tz_offset_minutes=480"))
            .expect("query should parse");
        let range = resolve_dashboard_range(&query).expect("range should resolve");
        assert_eq!(range.granularity, DashboardGranularity::Hour);
        assert!(range.previous.is_some());
    }
}
