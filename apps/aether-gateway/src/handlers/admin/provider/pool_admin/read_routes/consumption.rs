use super::{
    admin_pool_provider_id_from_consumption_path, build_admin_pool_error_response, pool_payloads,
    ADMIN_POOL_PROVIDER_CATALOG_READER_UNAVAILABLE_DETAIL,
    ADMIN_POOL_USAGE_READER_UNAVAILABLE_DETAIL,
};
use crate::handlers::admin::request::{AdminAppState, AdminRequestContext};
use crate::GatewayError;
use aether_admin::observability::stats::{
    parse_tz_offset_minutes, user_today, AdminStatsTimeRange,
};
use aether_data_contracts::repository::{
    provider_catalog::StoredProviderCatalogKey,
    usage::{ProviderApiKeyConsumptionSummaryQuery, StoredProviderApiKeyConsumptionSummary},
};
use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, NaiveDate};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct PoolConsumptionPeriodDef {
    key: &'static str,
    label: &'static str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default)]
struct PoolConsumptionAccount {
    key_id: String,
    key_name: String,
    auth_type: String,
    is_active: bool,
    account_quota: Option<String>,
    request_count: u64,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    total_cost_usd: f64,
}

impl PoolConsumptionAccount {
    fn cache_tokens(&self) -> u64 {
        self.cache_creation_tokens
            .saturating_add(self.cache_read_tokens)
    }

    fn to_json(&self) -> Value {
        json!({
            "key_id": self.key_id,
            "key_name": self.key_name,
            "auth_type": self.auth_type,
            "is_active": self.is_active,
            "account_quota": self.account_quota,
            "request_count": self.request_count,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "cache_creation_input_tokens": self.cache_creation_tokens,
            "cache_read_input_tokens": self.cache_read_tokens,
            "cache_tokens": self.cache_tokens(),
            "total_tokens": self.total_tokens,
            "total_cost_usd": format_pool_cost_usd(self.total_cost_usd),
        })
    }
}

pub(super) async fn build_admin_pool_consumption_stats_response(
    state: &AdminAppState<'_>,
    request_context: &AdminRequestContext<'_>,
) -> Result<Response<Body>, GatewayError> {
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

    let Some(provider_id) = admin_pool_provider_id_from_consumption_path(request_context.path())
    else {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::BAD_REQUEST,
            "provider_id 无效",
        ));
    };

    let tz_offset_minutes = match parse_tz_offset_minutes(request_context.query_string()) {
        Ok(value) => value,
        Err(detail) => {
            return Ok(build_admin_pool_error_response(
                http::StatusCode::BAD_REQUEST,
                detail,
            ));
        }
    };

    let Some(provider) = state
        .read_provider_catalog_providers_by_ids(std::slice::from_ref(&provider_id))
        .await?
        .into_iter()
        .next()
    else {
        return Ok(build_admin_pool_error_response(
            http::StatusCode::NOT_FOUND,
            format!("Provider {provider_id} 不存在"),
        ));
    };

    let provider_type = provider.provider_type.clone();
    let mut keys = state
        .list_provider_catalog_keys_by_provider_ids(std::slice::from_ref(&provider.id))
        .await?;
    keys.sort_by(|left, right| {
        left.internal_priority
            .cmp(&right.internal_priority)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });

    let periods = build_pool_consumption_periods(
        state,
        &provider.id,
        &provider_type,
        &keys,
        tz_offset_minutes,
    )
    .await?;

    Ok(Json(json!({
        "provider_id": provider.id,
        "provider_name": provider.name,
        "periods": periods,
    }))
    .into_response())
}

async fn build_pool_consumption_periods(
    state: &AdminAppState<'_>,
    provider_id: &str,
    provider_type: &str,
    keys: &[StoredProviderCatalogKey],
    tz_offset_minutes: i32,
) -> Result<Vec<Value>, GatewayError> {
    let mut periods = Vec::new();
    for period in resolve_pool_consumption_period_defs(tz_offset_minutes) {
        let bounds = build_pool_consumption_unix_bounds(&period, tz_offset_minutes);
        let query = build_provider_api_key_consumption_query(provider_id, keys, bounds);
        let aggregates = if let Some(query) = query.as_ref() {
            state.summarize_provider_api_key_consumption(query).await?
        } else {
            BTreeMap::new()
        };
        let accounts = build_pool_consumption_accounts(keys, provider_type, &aggregates);
        periods.push(json!({
            "key": period.key,
            "label": period.label,
            "start_date": period.start_date.map(|value| value.to_string()),
            "end_date": period.end_date.map(|value| value.to_string()),
            "summary": build_pool_consumption_summary_json(&accounts),
            "accounts": accounts.iter().map(PoolConsumptionAccount::to_json).collect::<Vec<_>>(),
        }));
    }
    Ok(periods)
}

fn build_provider_api_key_consumption_query(
    provider_id: &str,
    keys: &[StoredProviderCatalogKey],
    bounds: Option<(u64, u64)>,
) -> Option<ProviderApiKeyConsumptionSummaryQuery> {
    let global_start = bounds.map(|(start, _)| start).unwrap_or(0);
    let created_until_unix_secs = bounds.map(|(_, end)| end);
    let mut starts = BTreeMap::new();
    for key in keys {
        if created_until_unix_secs.is_some_and(|value| global_start >= value) {
            continue;
        }
        starts.insert(key.id.clone(), global_start);
    }
    if starts.is_empty() {
        return None;
    }
    Some(ProviderApiKeyConsumptionSummaryQuery {
        provider_id: provider_id.to_string(),
        created_until_unix_secs,
        created_from_unix_secs_by_provider_api_key_id: starts,
    })
}

fn resolve_pool_consumption_period_defs(tz_offset_minutes: i32) -> Vec<PoolConsumptionPeriodDef> {
    let today = user_today(tz_offset_minutes);
    vec![
        PoolConsumptionPeriodDef {
            key: "today",
            label: "今天",
            start_date: Some(today),
            end_date: Some(today),
        },
        PoolConsumptionPeriodDef {
            key: "last3days",
            label: "近 3 天",
            start_date: today.checked_sub_signed(Duration::days(2)),
            end_date: Some(today),
        },
        PoolConsumptionPeriodDef {
            key: "last7days",
            label: "近 7 天",
            start_date: today.checked_sub_signed(Duration::days(6)),
            end_date: Some(today),
        },
        PoolConsumptionPeriodDef {
            key: "last30days",
            label: "近 30 天",
            start_date: today.checked_sub_signed(Duration::days(29)),
            end_date: Some(today),
        },
        PoolConsumptionPeriodDef {
            key: "all",
            label: "全部",
            start_date: None,
            end_date: None,
        },
    ]
}

fn build_pool_consumption_unix_bounds(
    period: &PoolConsumptionPeriodDef,
    tz_offset_minutes: i32,
) -> Option<(u64, u64)> {
    let (start_date, end_date) = (period.start_date?, period.end_date?);
    AdminStatsTimeRange {
        start_date,
        end_date,
        tz_offset_minutes,
    }
    .to_unix_bounds()
}

fn build_pool_consumption_accounts(
    keys: &[StoredProviderCatalogKey],
    provider_type: &str,
    aggregates: &BTreeMap<String, StoredProviderApiKeyConsumptionSummary>,
) -> Vec<PoolConsumptionAccount> {
    let mut accounts = keys
        .iter()
        .filter_map(|key| {
            let aggregate = aggregates.get(&key.id)?;
            Some(PoolConsumptionAccount {
                key_id: key.id.clone(),
                key_name: key.name.clone(),
                auth_type: key.auth_type.clone(),
                is_active: key.is_active,
                account_quota: pool_payloads::admin_pool_account_quota_from_key(key, provider_type),
                request_count: aggregate.request_count,
                input_tokens: aggregate.input_tokens,
                output_tokens: aggregate.output_tokens,
                cache_creation_tokens: aggregate.cache_creation_tokens,
                cache_read_tokens: aggregate.cache_read_tokens,
                total_tokens: aggregate.total_tokens,
                total_cost_usd: aggregate.total_cost_usd,
            })
        })
        .collect::<Vec<_>>();

    accounts.sort_by(compare_pool_consumption_accounts_desc);
    accounts
}

fn build_pool_consumption_summary_json(accounts: &[PoolConsumptionAccount]) -> Value {
    if accounts.is_empty() {
        return json!({
            "account_count": 0,
            "request_count": 0,
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_tokens": 0,
            "total_tokens": 0,
            "total_cost_usd": format_pool_cost_usd(0.0),
            "avg_request_count": 0,
            "avg_input_tokens": 0,
            "avg_output_tokens": 0,
            "avg_cache_tokens": 0,
            "avg_total_tokens": 0,
            "avg_total_cost_usd": format_pool_cost_usd(0.0),
            "max_account": Value::Null,
            "min_account": Value::Null,
        });
    }

    let account_count = accounts.len() as u64;
    let request_count = accounts.iter().map(|item| item.request_count).sum::<u64>();
    let input_tokens = accounts.iter().map(|item| item.input_tokens).sum::<u64>();
    let output_tokens = accounts.iter().map(|item| item.output_tokens).sum::<u64>();
    let cache_tokens = accounts
        .iter()
        .map(PoolConsumptionAccount::cache_tokens)
        .sum::<u64>();
    let total_tokens = accounts.iter().map(|item| item.total_tokens).sum::<u64>();
    let total_cost_usd = accounts.iter().map(|item| item.total_cost_usd).sum::<f64>();
    let max_account = accounts
        .iter()
        .cloned()
        .max_by(compare_pool_consumption_accounts);
    let min_account = accounts
        .iter()
        .cloned()
        .min_by(compare_pool_consumption_accounts);

    json!({
        "account_count": account_count,
        "request_count": request_count,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cache_tokens": cache_tokens,
        "total_tokens": total_tokens,
        "total_cost_usd": format_pool_cost_usd(total_cost_usd),
        "avg_request_count": ((request_count as f64) / account_count as f64).round() as u64,
        "avg_input_tokens": ((input_tokens as f64) / account_count as f64).round() as u64,
        "avg_output_tokens": ((output_tokens as f64) / account_count as f64).round() as u64,
        "avg_cache_tokens": ((cache_tokens as f64) / account_count as f64).round() as u64,
        "avg_total_tokens": ((total_tokens as f64) / account_count as f64).round() as u64,
        "avg_total_cost_usd": format_pool_cost_usd(total_cost_usd / account_count as f64),
        "max_account": max_account.as_ref().map(PoolConsumptionAccount::to_json),
        "min_account": min_account.as_ref().map(PoolConsumptionAccount::to_json),
    })
}

fn compare_pool_consumption_accounts(
    left: &PoolConsumptionAccount,
    right: &PoolConsumptionAccount,
) -> Ordering {
    left.total_cost_usd
        .total_cmp(&right.total_cost_usd)
        .then_with(|| left.total_tokens.cmp(&right.total_tokens))
        .then_with(|| left.request_count.cmp(&right.request_count))
        .then_with(|| {
            normalize_pool_consumption_name(&left.key_name)
                .cmp(&normalize_pool_consumption_name(&right.key_name))
        })
}

fn compare_pool_consumption_accounts_desc(
    left: &PoolConsumptionAccount,
    right: &PoolConsumptionAccount,
) -> Ordering {
    right
        .total_cost_usd
        .total_cmp(&left.total_cost_usd)
        .then_with(|| right.total_tokens.cmp(&left.total_tokens))
        .then_with(|| right.request_count.cmp(&left.request_count))
        .then_with(|| {
            normalize_pool_consumption_name(&left.key_name)
                .cmp(&normalize_pool_consumption_name(&right.key_name))
        })
}

fn normalize_pool_consumption_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn format_pool_cost_usd(value: f64) -> String {
    let safe = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };
    format!("{safe:.8}")
}
