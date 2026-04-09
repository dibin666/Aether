use super::cache_types::AdminMonitoringCacheAffinityRecord;
use crate::cache::SchedulerAffinityTarget;
use crate::handlers::admin::request::AdminAppState;
use crate::GatewayError;
use std::time::Duration;

fn parse_admin_monitoring_cache_affinity_key(raw_key: &str) -> Option<(String, String, String)> {
    let parts = raw_key.split(':').collect::<Vec<_>>();
    let start = parts
        .iter()
        .position(|segment| *segment == "cache_affinity")?;
    let affinity_key = parts.get(start + 1)?.trim();
    if affinity_key.is_empty() {
        return None;
    }
    let api_format = parts
        .get(start + 2)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let model_name = parts
        .get(start + 3..)
        .filter(|segments| !segments.is_empty())
        .map(|segments| segments.join(":"))
        .unwrap_or_else(|| "unknown".to_string());
    Some((affinity_key.to_string(), api_format, model_name))
}

fn parse_admin_monitoring_scheduler_affinity_key(
    raw_key: &str,
) -> Option<(String, String, String)> {
    let parts = raw_key.split(':').collect::<Vec<_>>();
    let start = parts
        .iter()
        .position(|segment| *segment == "scheduler_affinity")?;
    let affinity_key = parts.get(start + 1)?.trim();
    if affinity_key.is_empty() {
        return None;
    }

    let remaining = parts.get(start + 2..)?;
    if remaining.len() < 2 {
        return None;
    }

    let (api_format, model_name_parts) = if remaining.len() == 2 {
        (remaining[0].trim().to_string(), &remaining[1..])
    } else {
        (
            format!("{}:{}", remaining[0].trim(), remaining[1].trim()),
            &remaining[2..],
        )
    };
    if api_format.trim().is_empty() {
        return None;
    }

    let model_name = model_name_parts
        .iter()
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(":");
    if model_name.is_empty() {
        return None;
    }

    Some((affinity_key.to_string(), api_format, model_name))
}

pub(super) fn admin_monitoring_scheduler_affinity_cache_key(
    record: &AdminMonitoringCacheAffinityRecord,
) -> Option<String> {
    let affinity_key = record.affinity_key.trim();
    let api_format = record.api_format.trim().to_ascii_lowercase();
    let model_name = record.model_name.trim();
    if affinity_key.is_empty() || api_format.is_empty() || model_name.is_empty() {
        return None;
    }
    Some(format!(
        "scheduler_affinity:{affinity_key}:{api_format}:{model_name}"
    ))
}

pub(super) fn admin_monitoring_cache_affinity_record(
    raw_key: &str,
    raw_value: &str,
) -> Option<AdminMonitoringCacheAffinityRecord> {
    let payload = serde_json::from_str::<serde_json::Value>(raw_value).ok()?;
    let object = payload.as_object()?;
    let (affinity_key, parsed_api_format, parsed_model_name) =
        parse_admin_monitoring_cache_affinity_key(raw_key)?;
    let api_format = object
        .get("api_format")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(parsed_api_format.as_str())
        .to_string();
    let model_name = object
        .get("model_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(parsed_model_name.as_str())
        .to_string();
    let request_count = object
        .get("request_count")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        })
        .unwrap_or(0);
    Some(AdminMonitoringCacheAffinityRecord {
        raw_key: raw_key.to_string(),
        affinity_key,
        api_format,
        model_name,
        provider_id: object
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        endpoint_id: object
            .get("endpoint_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        key_id: object
            .get("key_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        created_at: object.get("created_at").cloned(),
        expire_at: object.get("expire_at").cloned(),
        request_count,
    })
}

pub(super) fn admin_monitoring_scheduler_affinity_record(
    cache_key: &str,
    target: &SchedulerAffinityTarget,
    age: Duration,
    ttl: Duration,
    now_unix_secs: u64,
) -> Option<AdminMonitoringCacheAffinityRecord> {
    let (affinity_key, api_format, model_name) =
        parse_admin_monitoring_scheduler_affinity_key(cache_key)?;
    let age_secs = age.as_secs();
    let created_at = now_unix_secs.saturating_sub(age_secs);
    let expire_at = created_at.saturating_add(ttl.as_secs());

    Some(AdminMonitoringCacheAffinityRecord {
        raw_key: cache_key.to_string(),
        affinity_key,
        api_format,
        model_name,
        provider_id: Some(target.provider_id.clone()),
        endpoint_id: Some(target.endpoint_id.clone()),
        key_id: Some(target.key_id.clone()),
        created_at: Some(serde_json::json!(created_at)),
        expire_at: Some(serde_json::json!(expire_at)),
        request_count: 0,
    })
}

pub(super) fn admin_monitoring_scheduler_affinity_record_from_raw(
    raw_key: &str,
    raw_value: &str,
) -> Option<AdminMonitoringCacheAffinityRecord> {
    let payload = serde_json::from_str::<serde_json::Value>(raw_value).ok()?;
    let object = payload.as_object()?;
    let (affinity_key, parsed_api_format, parsed_model_name) =
        parse_admin_monitoring_scheduler_affinity_key(raw_key)?;
    let api_format = object
        .get("api_format")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(parsed_api_format.as_str())
        .to_string();
    let model_name = object
        .get("model_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(parsed_model_name.as_str())
        .to_string();
    let request_count = object
        .get("request_count")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        })
        .unwrap_or(0);

    Some(AdminMonitoringCacheAffinityRecord {
        raw_key: raw_key.to_string(),
        affinity_key,
        api_format,
        model_name,
        provider_id: object
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        endpoint_id: object
            .get("endpoint_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        key_id: object
            .get("key_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        created_at: object.get("created_at").cloned(),
        expire_at: object.get("expire_at").cloned(),
        request_count,
    })
}

pub(super) fn admin_monitoring_cache_affinity_record_identity(
    record: &AdminMonitoringCacheAffinityRecord,
) -> String {
    admin_monitoring_scheduler_affinity_cache_key(record).unwrap_or_else(|| record.raw_key.clone())
}

pub(super) fn clear_admin_monitoring_scheduler_affinity_entries(
    state: &AdminAppState<'_>,
    records: &[AdminMonitoringCacheAffinityRecord],
) {
    let scheduler_keys = records
        .iter()
        .filter_map(admin_monitoring_scheduler_affinity_cache_key)
        .collect::<std::collections::BTreeSet<_>>();
    for scheduler_key in scheduler_keys {
        let _ = state
            .as_ref()
            .remove_scheduler_affinity_cache_entry(&scheduler_key);
    }
}

#[cfg(test)]
pub(super) fn delete_admin_monitoring_cache_affinity_entries_for_tests(
    state: &AdminAppState<'_>,
    raw_keys: &[String],
) -> usize {
    state
        .as_ref()
        .remove_admin_monitoring_cache_affinity_entries_for_tests(raw_keys)
}

#[cfg(not(test))]
pub(super) fn delete_admin_monitoring_cache_affinity_entries_for_tests(
    _state: &AdminAppState<'_>,
    _raw_keys: &[String],
) -> usize {
    0
}

pub(super) async fn delete_admin_monitoring_cache_affinity_raw_keys(
    state: &AdminAppState<'_>,
    raw_keys: &[String],
) -> Result<usize, GatewayError> {
    if raw_keys.is_empty() {
        return Ok(0);
    }

    if let Some(runner) = state.redis_kv_runner() {
        let mut connection = runner
            .client()
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| {
                GatewayError::Internal(format!("admin monitoring redis connect failed: {err}"))
            })?;
        let deleted = redis::cmd("DEL")
            .arg(raw_keys)
            .query_async::<i64>(&mut connection)
            .await
            .map_err(|err| {
                GatewayError::Internal(format!("admin monitoring redis delete failed: {err}"))
            })?;
        return Ok(usize::try_from(deleted).unwrap_or(0));
    }

    Ok(delete_admin_monitoring_cache_affinity_entries_for_tests(
        state, raw_keys,
    ))
}
