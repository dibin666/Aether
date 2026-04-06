use crate::handlers::admin::shared::unix_secs_to_rfc3339;
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

pub(super) fn key_api_formats_without_entry(
    key: &StoredProviderCatalogKey,
    api_format: &str,
) -> Option<Vec<String>> {
    let current_formats =
        crate::handlers::admin::shared::json_string_list(key.api_formats.as_ref());
    if !current_formats
        .iter()
        .any(|candidate| candidate == api_format)
    {
        return None;
    }
    Some(
        current_formats
            .into_iter()
            .filter(|candidate| candidate != api_format)
            .collect(),
    )
}

pub(super) fn endpoint_key_counts_by_format(
    keys: &[StoredProviderCatalogKey],
) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let mut total = BTreeMap::new();
    let mut active = BTreeMap::new();
    for key in keys {
        let Some(formats) = key
            .api_formats
            .as_ref()
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for api_format in formats.iter().filter_map(serde_json::Value::as_str) {
            *total.entry(api_format.to_string()).or_insert(0) += 1;
            if key.is_active {
                *active.entry(api_format.to_string()).or_insert(0) += 1;
            }
        }
    }
    (total, active)
}

pub(super) fn build_admin_provider_endpoint_response(
    endpoint: &StoredProviderCatalogEndpoint,
    provider_name: &str,
    total_keys: usize,
    active_keys: usize,
    now_unix_secs: u64,
) -> serde_json::Value {
    json!({
        "id": endpoint.id,
        "provider_id": endpoint.provider_id,
        "provider_name": provider_name,
        "api_format": endpoint.api_format,
        "base_url": endpoint.base_url,
        "custom_path": endpoint.custom_path,
        "header_rules": endpoint.header_rules,
        "body_rules": endpoint.body_rules,
        "max_retries": endpoint.max_retries.unwrap_or(2),
        "is_active": endpoint.is_active,
        "config": endpoint.config,
        "proxy": masked_proxy_value(endpoint.proxy.as_ref()),
        "format_acceptance_config": endpoint.format_acceptance_config,
        "total_keys": total_keys,
        "active_keys": active_keys,
        "created_at": endpoint_timestamp_or_now(endpoint.created_at_unix_secs, now_unix_secs),
        "updated_at": endpoint_timestamp_or_now(endpoint.updated_at_unix_secs, now_unix_secs),
    })
}

fn masked_proxy_value(proxy: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(proxy) = proxy.and_then(serde_json::Value::as_object) else {
        return serde_json::Value::Null;
    };
    let mut masked = proxy.clone();
    if masked
        .get("password")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        masked.insert("password".to_string(), json!("***"));
    }
    serde_json::Value::Object(masked)
}

fn endpoint_timestamp_or_now(value: Option<u64>, now_unix_secs: u64) -> serde_json::Value {
    unix_secs_to_rfc3339(value.unwrap_or(now_unix_secs))
        .map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null)
}

fn default_admin_endpoint_max_retries() -> i32 {
    2
}

#[derive(Debug, Deserialize)]
pub(super) struct AdminProviderEndpointCreateRequest {
    pub(super) provider_id: String,
    pub(super) api_format: String,
    pub(super) base_url: String,
    #[serde(default)]
    pub(super) custom_path: Option<String>,
    #[serde(default)]
    pub(super) header_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) body_rules: Option<serde_json::Value>,
    #[serde(default = "default_admin_endpoint_max_retries")]
    pub(super) max_retries: i32,
    #[serde(default)]
    pub(super) config: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) format_acceptance_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdminProviderEndpointUpdateRequest {
    #[serde(default)]
    pub(super) base_url: Option<String>,
    #[serde(default)]
    pub(super) custom_path: Option<String>,
    #[serde(default)]
    pub(super) header_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) body_rules: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) max_retries: Option<i32>,
    #[serde(default)]
    pub(super) is_active: Option<bool>,
    #[serde(default)]
    pub(super) config: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) format_acceptance_config: Option<serde_json::Value>,
}
