use crate::api::ai::admin_endpoint_signature_parts;
use crate::handlers::shared::{decrypt_catalog_secret_with_fallbacks, unix_secs_to_rfc3339};
use crate::{AppState, GatewayError};
use aether_crypto::encrypt_python_fernet_plaintext;
use aether_data_contracts::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery,
};
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint;
use axum::body::Bytes;
use axum::http;
use chrono::Utc;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const ADMIN_SYSTEM_CONFIG_EXPORT_VERSION: &str = "2.2";
const ADMIN_SYSTEM_EXPORT_PAGE_LIMIT: usize = 10_000;
const PROVIDER_OPS_SENSITIVE_CREDENTIAL_FIELDS: &[&str] = &[
    "api_key",
    "password",
    "refresh_token",
    "session_token",
    "session_cookie",
    "token_cookie",
    "auth_cookie",
    "cookie_string",
    "cookie",
];
const REQUEST_RECORD_LEVEL_KEY: &str = "request_record_level";
const LEGACY_REQUEST_LOG_LEVEL_KEY: &str = "request_log_level";
const SENSITIVE_SYSTEM_CONFIG_KEYS: &[&str] = &["smtp_password"];
const ADMIN_SYSTEM_USERS_EXPORT_VERSION: &str = "1.3";

pub(crate) fn decrypt_admin_system_export_secret(
    state: &AppState,
    ciphertext: &str,
) -> Option<String> {
    decrypt_catalog_secret_with_fallbacks(state.encryption_key(), ciphertext)
}

pub(crate) fn normalize_admin_system_export_api_formats(
    raw_formats: Option<&serde_json::Value>,
) -> Vec<String> {
    let Some(raw_formats) = raw_formats.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in raw_formats {
        let Some(value) = raw
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some((signature, _, _)) = admin_endpoint_signature_parts(value) else {
            continue;
        };
        if seen.insert(signature) {
            normalized.push(signature.to_string());
        }
    }
    normalized
}

pub(crate) fn resolve_admin_system_export_key_api_formats(
    raw_formats: Option<&serde_json::Value>,
    provider_endpoint_formats: &[String],
) -> Vec<String> {
    let normalized = normalize_admin_system_export_api_formats(raw_formats);
    if !normalized.is_empty() {
        return normalized;
    }
    if raw_formats.is_none() {
        return provider_endpoint_formats.to_vec();
    }
    Vec::new()
}

pub(crate) fn collect_admin_system_export_provider_endpoint_formats(
    endpoints: &[StoredProviderCatalogEndpoint],
) -> Vec<String> {
    endpoints
        .iter()
        .filter_map(|endpoint| admin_endpoint_signature_parts(&endpoint.api_format))
        .map(|(signature, _, _)| signature.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn decrypt_admin_system_export_provider_config(
    state: &AppState,
    config: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut decrypted = config.cloned()?;
    let Some(credentials) = decrypted
        .get_mut("provider_ops")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|provider_ops| provider_ops.get_mut("connector"))
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|connector| connector.get_mut("credentials"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Some(decrypted);
    };

    for field in PROVIDER_OPS_SENSITIVE_CREDENTIAL_FIELDS {
        let Some(serde_json::Value::String(ciphertext)) = credentials.get(*field).cloned() else {
            continue;
        };
        if let Some(plaintext) = decrypt_admin_system_export_secret(state, &ciphertext) {
            credentials.insert((*field).to_string(), serde_json::Value::String(plaintext));
        }
    }

    Some(decrypted)
}

pub(crate) async fn build_admin_system_config_export_payload(
    state: &AppState,
) -> Result<serde_json::Value, GatewayError> {
    let global_models = state
        .list_admin_global_models(&AdminGlobalModelListQuery {
            offset: 0,
            limit: ADMIN_SYSTEM_EXPORT_PAGE_LIMIT,
            is_active: None,
            search: None,
        })
        .await?
        .items;
    let global_model_name_by_id = global_models
        .iter()
        .map(|model| (model.id.clone(), model.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let global_models_data = global_models
        .iter()
        .map(|model| {
            json!({
                "name": model.name,
                "display_name": model.display_name,
                "default_price_per_request": model.default_price_per_request,
                "default_tiered_pricing": model.default_tiered_pricing,
                "supported_capabilities": model.supported_capabilities,
                "config": model.config,
                "is_active": model.is_active,
            })
        })
        .collect::<Vec<_>>();

    let providers = state.list_provider_catalog_providers(false).await?;
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let endpoints = state
        .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
        .await?;
    let keys = state
        .list_provider_catalog_keys_by_provider_ids(&provider_ids)
        .await?;

    let mut endpoints_by_provider = BTreeMap::<String, Vec<_>>::new();
    for endpoint in endpoints {
        endpoints_by_provider
            .entry(endpoint.provider_id.clone())
            .or_default()
            .push(endpoint);
    }
    let mut keys_by_provider = BTreeMap::<String, Vec<_>>::new();
    for key in keys {
        keys_by_provider
            .entry(key.provider_id.clone())
            .or_default()
            .push(key);
    }

    let mut provider_models_by_provider = BTreeMap::<String, Vec<_>>::new();
    for provider in &providers {
        let models = state
            .list_admin_provider_models(&AdminProviderModelListQuery {
                provider_id: provider.id.clone(),
                is_active: None,
                offset: 0,
                limit: ADMIN_SYSTEM_EXPORT_PAGE_LIMIT,
            })
            .await?;
        provider_models_by_provider.insert(provider.id.clone(), models);
    }

    let providers_data = providers
        .iter()
        .map(|provider| {
            let endpoints = endpoints_by_provider.remove(&provider.id).unwrap_or_default();
            let provider_endpoint_formats =
                collect_admin_system_export_provider_endpoint_formats(&endpoints);
            let endpoints_data = endpoints
                .iter()
                .map(|endpoint| {
                    json!({
                        "api_format": endpoint.api_format,
                        "base_url": endpoint.base_url,
                        "header_rules": endpoint.header_rules,
                        "body_rules": endpoint.body_rules,
                        "max_retries": endpoint.max_retries,
                        "is_active": endpoint.is_active,
                        "custom_path": endpoint.custom_path,
                        "config": endpoint.config,
                        "format_acceptance_config": endpoint.format_acceptance_config,
                        "proxy": endpoint.proxy,
                    })
                })
                .collect::<Vec<_>>();

            let mut keys = keys_by_provider.remove(&provider.id).unwrap_or_default();
            keys.sort_by(|left, right| {
                left.internal_priority
                    .cmp(&right.internal_priority)
                    .then(
                        left.created_at_unix_secs
                            .unwrap_or(0)
                            .cmp(&right.created_at_unix_secs.unwrap_or(0)),
                    )
                    .then(left.id.cmp(&right.id))
            });
            let keys_data = keys
                .iter()
                .map(|key| {
                    let api_formats = resolve_admin_system_export_key_api_formats(
                        key.api_formats.as_ref(),
                        &provider_endpoint_formats,
                    );
                    let mut payload = json!({
                        "api_formats": api_formats,
                        "supported_endpoints": api_formats,
                        "auth_type": key.auth_type,
                        "name": key.name,
                        "note": key.note,
                        "rate_multipliers": key.rate_multipliers,
                        "internal_priority": key.internal_priority,
                        "global_priority_by_format": key.global_priority_by_format,
                        "rpm_limit": key.rpm_limit,
                        "allowed_models": key.allowed_models,
                        "capabilities": key.capabilities,
                        "cache_ttl_minutes": key.cache_ttl_minutes,
                        "max_probe_interval_minutes": key.max_probe_interval_minutes,
                        "is_active": key.is_active,
                        "proxy": key.proxy,
                        "fingerprint": key.fingerprint,
                        "auto_fetch_models": key.auto_fetch_models,
                        "locked_models": key.locked_models,
                        "model_include_patterns": key.model_include_patterns,
                        "model_exclude_patterns": key.model_exclude_patterns,
                        "api_key": decrypt_admin_system_export_secret(state, &key.encrypted_api_key)
                            .unwrap_or_default(),
                    });
                    if let Some(ciphertext) = key.encrypted_auth_config.as_deref() {
                        if let Some(plaintext) =
                            decrypt_admin_system_export_secret(state, ciphertext)
                        {
                            payload["auth_config"] = json!(plaintext);
                        }
                    }
                    payload
                })
                .collect::<Vec<_>>();

            let models_data = provider_models_by_provider
                .remove(&provider.id)
                .unwrap_or_default()
                .into_iter()
                .map(|model| {
                    json!({
                        "provider_model_name": model.provider_model_name,
                        "provider_model_mappings": model.provider_model_mappings,
                        "price_per_request": model.price_per_request,
                        "tiered_pricing": model.tiered_pricing,
                        "supports_vision": model.supports_vision,
                        "supports_function_calling": model.supports_function_calling,
                        "supports_streaming": model.supports_streaming,
                        "supports_extended_thinking": model.supports_extended_thinking,
                        "supports_image_generation": model.supports_image_generation,
                        "is_active": model.is_active,
                        "config": model.config,
                        "global_model_name": global_model_name_by_id.get(&model.global_model_id),
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "name": provider.name,
                "description": provider.description,
                "website": provider.website,
                "provider_type": provider.provider_type,
                "billing_type": provider.billing_type,
                "monthly_quota_usd": provider.monthly_quota_usd,
                "quota_reset_day": provider.quota_reset_day,
                "provider_priority": provider.provider_priority,
                "keep_priority_on_conversion": provider.keep_priority_on_conversion,
                "enable_format_conversion": provider.enable_format_conversion,
                "is_active": provider.is_active,
                "concurrent_limit": provider.concurrent_limit,
                "max_retries": provider.max_retries,
                "proxy": provider.proxy,
                "request_timeout": provider.request_timeout_secs,
                "stream_first_byte_timeout": provider.stream_first_byte_timeout_secs,
                "config": decrypt_admin_system_export_provider_config(state, provider.config.as_ref()),
                "endpoints": endpoints_data,
                "api_keys": keys_data,
                "models": models_data,
            })
        })
        .collect::<Vec<_>>();

    let ldap_config = state.get_ldap_module_config().await?;
    let ldap_data = ldap_config.map(|config| {
        let bind_password = config
            .bind_password_encrypted
            .as_deref()
            .and_then(|ciphertext| decrypt_admin_system_export_secret(state, ciphertext))
            .unwrap_or_default();
        json!({
            "server_url": config.server_url,
            "bind_dn": config.bind_dn,
            "bind_password": bind_password,
            "base_dn": config.base_dn,
            "user_search_filter": config.user_search_filter,
            "username_attr": config.username_attr,
            "email_attr": config.email_attr,
            "display_name_attr": config.display_name_attr,
            "is_enabled": config.is_enabled,
            "is_exclusive": config.is_exclusive,
            "use_starttls": config.use_starttls,
            "connect_timeout": config.connect_timeout,
        })
    });

    let system_configs = state.list_system_config_entries().await?;
    let system_configs_data = system_configs
        .iter()
        .map(|entry| {
            let value = if is_sensitive_admin_system_config_key(&entry.key) {
                entry
                    .value
                    .as_str()
                    .and_then(|ciphertext| decrypt_admin_system_export_secret(state, ciphertext))
                    .map(serde_json::Value::String)
                    .unwrap_or_else(|| entry.value.clone())
            } else {
                entry.value.clone()
            };
            json!({
                "key": entry.key,
                "value": value,
                "description": entry.description,
            })
        })
        .collect::<Vec<_>>();

    let oauth_providers = state.list_oauth_provider_configs().await?;
    let oauth_data = oauth_providers
        .iter()
        .map(|provider| {
            let client_secret = provider
                .client_secret_encrypted
                .as_deref()
                .and_then(|ciphertext| decrypt_admin_system_export_secret(state, ciphertext))
                .unwrap_or_default();
            json!({
                "provider_type": provider.provider_type,
                "display_name": provider.display_name,
                "client_id": provider.client_id,
                "client_secret": client_secret,
                "authorization_url_override": provider.authorization_url_override,
                "token_url_override": provider.token_url_override,
                "userinfo_url_override": provider.userinfo_url_override,
                "scopes": provider.scopes,
                "redirect_uri": provider.redirect_uri,
                "frontend_callback_url": provider.frontend_callback_url,
                "attribute_mapping": provider.attribute_mapping,
                "extra_config": provider.extra_config,
                "is_enabled": provider.is_enabled,
            })
        })
        .collect::<Vec<_>>();

    let proxy_nodes = state.list_proxy_nodes().await?;
    let proxy_nodes_data = proxy_nodes
        .iter()
        .map(|node| {
            json!({
                "id": node.id,
                "name": node.name,
                "ip": node.ip,
                "port": node.port,
                "region": node.region,
                "is_manual": node.is_manual,
                "proxy_url": node.proxy_url,
                "proxy_username": node.proxy_username,
                "proxy_password": node.proxy_password,
                "tunnel_mode": node.tunnel_mode,
                "heartbeat_interval": node.heartbeat_interval,
                "remote_config": node.remote_config,
                "config_version": node.config_version,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "version": ADMIN_SYSTEM_CONFIG_EXPORT_VERSION,
        "exported_at": Utc::now().to_rfc3339(),
        "global_models": global_models_data,
        "providers": providers_data,
        "proxy_nodes": proxy_nodes_data,
        "ldap_config": ldap_data,
        "oauth_providers": oauth_data,
        "system_configs": system_configs_data,
    }))
}

pub(crate) fn serialize_admin_system_users_export_wallet(
    wallet: Option<&aether_data::repository::wallet::StoredWalletSnapshot>,
) -> Option<serde_json::Value> {
    let wallet = wallet?;
    let recharge_balance = wallet.balance;
    let gift_balance = wallet.gift_balance;
    let spendable_balance = recharge_balance + gift_balance;
    let unlimited = wallet.limit_mode.eq_ignore_ascii_case("unlimited");

    Some(json!({
        "id": wallet.id.clone(),
        "balance": spendable_balance,
        "recharge_balance": recharge_balance,
        "gift_balance": gift_balance,
        "refundable_balance": recharge_balance,
        "currency": wallet.currency.clone(),
        "status": wallet.status.clone(),
        "limit_mode": wallet.limit_mode.clone(),
        "unlimited": unlimited,
        "total_recharged": wallet.total_recharged,
        "total_consumed": wallet.total_consumed,
        "total_refunded": wallet.total_refunded,
        "total_adjusted": wallet.total_adjusted,
        "updated_at": unix_secs_to_rfc3339(wallet.updated_at_unix_secs),
    }))
}

fn build_admin_system_users_export_api_key_payload(
    state: &AppState,
    key: &aether_data::repository::auth::StoredAuthApiKeyExportRecord,
    wallet: Option<&aether_data::repository::wallet::StoredWalletSnapshot>,
    include_is_standalone: bool,
) -> serde_json::Value {
    let mut payload = serde_json::Map::from_iter([
        ("key_hash".to_string(), json!(key.key_hash.clone())),
        ("name".to_string(), json!(key.name.clone())),
        (
            "allowed_providers".to_string(),
            json!(key.allowed_providers.clone()),
        ),
        (
            "allowed_api_formats".to_string(),
            json!(key.allowed_api_formats.clone()),
        ),
        (
            "allowed_models".to_string(),
            json!(key.allowed_models.clone()),
        ),
        ("rate_limit".to_string(), json!(key.rate_limit)),
        ("concurrent_limit".to_string(), json!(key.concurrent_limit)),
        (
            "force_capabilities".to_string(),
            json!(key.force_capabilities.clone()),
        ),
        ("is_active".to_string(), json!(key.is_active)),
        (
            "expires_at".to_string(),
            json!(key.expires_at_unix_secs.and_then(unix_secs_to_rfc3339)),
        ),
        (
            "auto_delete_on_expiry".to_string(),
            json!(key.auto_delete_on_expiry),
        ),
        ("total_requests".to_string(), json!(key.total_requests)),
        ("total_cost_usd".to_string(), json!(key.total_cost_usd)),
        (
            "wallet".to_string(),
            serialize_admin_system_users_export_wallet(wallet).unwrap_or(serde_json::Value::Null),
        ),
    ]);

    if let Some(ciphertext) = key.key_encrypted.as_deref() {
        if let Some(plaintext) = decrypt_admin_system_export_secret(state, ciphertext) {
            payload.insert("key".to_string(), serde_json::Value::String(plaintext));
        } else {
            payload.insert(
                "key_encrypted".to_string(),
                serde_json::Value::String(ciphertext.to_string()),
            );
        }
    }

    if include_is_standalone {
        payload.insert("is_standalone".to_string(), json!(key.is_standalone));
    }

    serde_json::Value::Object(payload)
}

pub(crate) async fn build_admin_system_users_export_payload(
    state: &AppState,
) -> Result<serde_json::Value, GatewayError> {
    let users = state.list_non_admin_export_users().await?;
    let user_ids = users.iter().map(|user| user.id.clone()).collect::<Vec<_>>();
    let user_wallets = state.list_wallet_snapshots_by_user_ids(&user_ids).await?;
    let user_api_keys = state
        .list_auth_api_key_export_records_by_user_ids(&user_ids)
        .await?;
    let standalone_api_keys = state.list_auth_api_key_export_standalone_records().await?;
    let standalone_api_key_ids = standalone_api_keys
        .iter()
        .map(|key| key.api_key_id.clone())
        .collect::<Vec<_>>();
    let standalone_wallets = state
        .list_wallet_snapshots_by_api_key_ids(&standalone_api_key_ids)
        .await?;

    let wallets_by_user_id = user_wallets
        .into_iter()
        .filter_map(|wallet| wallet.user_id.clone().map(|user_id| (user_id, wallet)))
        .collect::<BTreeMap<_, _>>();
    let wallets_by_api_key_id = standalone_wallets
        .into_iter()
        .filter_map(|wallet| {
            wallet
                .api_key_id
                .clone()
                .map(|api_key_id| (api_key_id, wallet))
        })
        .collect::<BTreeMap<_, _>>();

    let mut api_keys_by_user_id =
        BTreeMap::<String, Vec<aether_data::repository::auth::StoredAuthApiKeyExportRecord>>::new();
    for key in user_api_keys.into_iter().filter(|key| !key.is_standalone) {
        api_keys_by_user_id
            .entry(key.user_id.clone())
            .or_default()
            .push(key);
    }

    let users_data = users
        .iter()
        .map(|user| {
            let wallet = wallets_by_user_id.get(&user.id);
            let wallet_payload = serialize_admin_system_users_export_wallet(wallet);
            let api_keys = api_keys_by_user_id.remove(&user.id).unwrap_or_default();
            let api_keys_payload = api_keys
                .iter()
                .map(|key| build_admin_system_users_export_api_key_payload(state, key, None, true))
                .collect::<Vec<_>>();

            json!({
                "email": user.email.clone(),
                "email_verified": user.email_verified,
                "username": user.username.clone(),
                "password_hash": user.password_hash.clone(),
                "role": user.role.clone(),
                "allowed_providers": user.allowed_providers.clone(),
                "allowed_api_formats": user.allowed_api_formats.clone(),
                "allowed_models": user.allowed_models.clone(),
                "rate_limit": user.rate_limit,
                "model_capability_settings": user.model_capability_settings.clone(),
                "unlimited": wallet
                    .map(|entry| entry.limit_mode.eq_ignore_ascii_case("unlimited"))
                    .unwrap_or(false),
                "wallet": wallet_payload,
                "is_active": user.is_active,
                "api_keys": api_keys_payload,
            })
        })
        .collect::<Vec<_>>();

    let standalone_keys_data = standalone_api_keys
        .iter()
        .map(|key| {
            build_admin_system_users_export_api_key_payload(
                state,
                key,
                wallets_by_api_key_id.get(&key.api_key_id),
                false,
            )
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "version": ADMIN_SYSTEM_USERS_EXPORT_VERSION,
        "exported_at": Utc::now().to_rfc3339(),
        "users": users_data,
        "standalone_keys": standalone_keys_data,
    }))
}

fn normalize_admin_system_config_key(requested_key: &str) -> String {
    let trimmed = requested_key.trim();
    if trimmed.eq_ignore_ascii_case(LEGACY_REQUEST_LOG_LEVEL_KEY) {
        REQUEST_RECORD_LEVEL_KEY.to_string()
    } else {
        trimmed.to_string()
    }
}

fn admin_system_config_delete_keys(requested_key: &str) -> Vec<String> {
    let normalized = normalize_admin_system_config_key(requested_key);
    if normalized == REQUEST_RECORD_LEVEL_KEY {
        vec![
            REQUEST_RECORD_LEVEL_KEY.to_string(),
            LEGACY_REQUEST_LOG_LEVEL_KEY.to_string(),
        ]
    } else {
        vec![normalized]
    }
}

fn is_sensitive_admin_system_config_key(key: &str) -> bool {
    SENSITIVE_SYSTEM_CONFIG_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn system_config_is_set(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(|value| value != 0)
            .or_else(|| value.as_u64().map(|value| value != 0))
            .or_else(|| value.as_f64().map(|value| value != 0.0))
            .unwrap_or(false),
        serde_json::Value::String(value) => !value.trim().is_empty(),
        serde_json::Value::Array(value) => !value.is_empty(),
        serde_json::Value::Object(value) => !value.is_empty(),
    }
}

fn admin_system_config_default_value(key: &str) -> Option<serde_json::Value> {
    match key {
        "site_name" => Some(json!("Aether")),
        "site_subtitle" => Some(json!("AI Gateway")),
        "default_user_initial_gift_usd" => Some(json!(10.0)),
        "password_policy_level" => Some(json!("weak")),
        REQUEST_RECORD_LEVEL_KEY => Some(json!("basic")),
        "max_request_body_size" => Some(json!(5_242_880)),
        "max_response_body_size" => Some(json!(5_242_880)),
        "sensitive_headers" => Some(json!([
            "authorization",
            "x-api-key",
            "api-key",
            "cookie",
            "set-cookie"
        ])),
        "detail_log_retention_days" => Some(json!(7)),
        "compressed_log_retention_days" => Some(json!(30)),
        "header_retention_days" => Some(json!(90)),
        "log_retention_days" => Some(json!(365)),
        "enable_auto_cleanup" => Some(json!(true)),
        "cleanup_batch_size" => Some(json!(1000)),
        "request_candidates_retention_days" => Some(json!(30)),
        "request_candidates_cleanup_batch_size" => Some(json!(5000)),
        "enable_provider_checkin" => Some(json!(true)),
        "provider_checkin_time" => Some(json!("01:05")),
        "provider_priority_mode" => Some(json!("provider")),
        "scheduling_mode" => Some(json!("cache_affinity")),
        "auto_delete_expired_keys" => Some(json!(false)),
        "email_suffix_mode" => Some(json!("none")),
        "email_suffix_list" => Some(json!([])),
        "enable_format_conversion" => Some(json!(true)),
        "keep_priority_on_conversion" => Some(json!(false)),
        "audit_log_retention_days" => Some(json!(30)),
        "enable_db_maintenance" => Some(json!(true)),
        "system_proxy_node_id" => Some(serde_json::Value::Null),
        "smtp_host" => Some(serde_json::Value::Null),
        "smtp_port" => Some(json!(587)),
        "smtp_user" => Some(serde_json::Value::Null),
        "smtp_password" => Some(serde_json::Value::Null),
        "smtp_use_tls" => Some(json!(true)),
        "smtp_use_ssl" => Some(json!(false)),
        "smtp_from_email" => Some(serde_json::Value::Null),
        "smtp_from_name" => Some(json!("Aether")),
        "enable_oauth_token_refresh" => Some(json!(true)),
        _ => None,
    }
}

fn build_admin_system_config_list_item(
    key: &str,
    value: &serde_json::Value,
    description: Option<&str>,
    updated_at_unix_secs: Option<u64>,
) -> serde_json::Value {
    let masked_value = if is_sensitive_admin_system_config_key(key) {
        serde_json::Value::Null
    } else {
        value.clone()
    };
    let is_set = is_sensitive_admin_system_config_key(key).then(|| system_config_is_set(value));
    let mut payload = json!({
        "key": key,
        "description": description,
        "updated_at": updated_at_unix_secs.and_then(unix_secs_to_rfc3339),
        "value": masked_value,
    });
    if let Some(is_set) = is_set {
        payload["is_set"] = json!(is_set);
    }
    payload
}

pub(crate) fn build_admin_system_configs_payload(
    entries: &[aether_data::repository::system::StoredSystemConfigEntry],
) -> serde_json::Value {
    let has_request_record_level = entries
        .iter()
        .any(|entry| entry.key == REQUEST_RECORD_LEVEL_KEY);
    json!(entries
        .iter()
        .filter_map(|entry| {
            if entry.key == LEGACY_REQUEST_LOG_LEVEL_KEY && has_request_record_level {
                return None;
            }
            let key = if entry.key == LEGACY_REQUEST_LOG_LEVEL_KEY {
                REQUEST_RECORD_LEVEL_KEY
            } else {
                entry.key.as_str()
            };
            Some(build_admin_system_config_list_item(
                key,
                &entry.value,
                entry.description.as_deref(),
                entry.updated_at_unix_secs,
            ))
        })
        .collect::<Vec<_>>())
}

pub(crate) async fn build_admin_system_config_detail_payload(
    state: &AppState,
    requested_key: &str,
) -> Result<Result<serde_json::Value, (http::StatusCode, serde_json::Value)>, GatewayError> {
    let requested_key = requested_key.trim();
    let normalized_key = normalize_admin_system_config_key(requested_key);
    let value = state
        .read_system_config_json_value(&normalized_key)
        .await?
        .or_else(|| admin_system_config_default_value(&normalized_key));
    let Some(value) = value else {
        return Ok(Err((
            http::StatusCode::NOT_FOUND,
            json!({ "detail": format!("配置项 '{requested_key}' 不存在") }),
        )));
    };
    if is_sensitive_admin_system_config_key(&normalized_key) {
        return Ok(Ok(json!({
            "key": requested_key,
            "value": serde_json::Value::Null,
            "is_set": system_config_is_set(&value),
        })));
    }
    Ok(Ok(json!({
        "key": requested_key,
        "value": value,
    })))
}

pub(crate) async fn apply_admin_system_config_update(
    state: &AppState,
    requested_key: &str,
    request_body: &Bytes,
) -> Result<Result<serde_json::Value, (http::StatusCode, serde_json::Value)>, GatewayError> {
    let payload = match serde_json::from_slice::<serde_json::Value>(request_body) {
        Ok(serde_json::Value::Object(payload)) => payload,
        _ => {
            return Ok(Err((
                http::StatusCode::BAD_REQUEST,
                json!({ "detail": "请求数据验证失败" }),
            )));
        }
    };
    let normalized_key = normalize_admin_system_config_key(requested_key);
    let mut value = payload
        .get("value")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let description = match payload.get("description") {
        Some(serde_json::Value::String(value)) => Some(value.trim().to_string()),
        Some(serde_json::Value::Null) | None => None,
        Some(_) => {
            return Ok(Err((
                http::StatusCode::BAD_REQUEST,
                json!({ "detail": "请求数据验证失败" }),
            )));
        }
    };

    if normalized_key == "password_policy_level" {
        match value.as_str().map(str::trim) {
            Some("weak" | "medium" | "strong") => {
                value = json!(value.as_str().unwrap().trim());
            }
            Some(_) => {
                return Ok(Err((
                    http::StatusCode::BAD_REQUEST,
                    json!({ "detail": "请求数据验证失败" }),
                )));
            }
            None if value.is_null() => {
                value = json!("weak");
            }
            None => {
                return Ok(Err((
                    http::StatusCode::BAD_REQUEST,
                    json!({ "detail": "请求数据验证失败" }),
                )));
            }
        }
    }

    if is_sensitive_admin_system_config_key(&normalized_key)
        && value.as_str().is_some_and(|raw| !raw.is_empty())
    {
        let Some(encryption_key) = state
            .encryption_key()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(Err((
                http::StatusCode::SERVICE_UNAVAILABLE,
                json!({ "detail": "系统配置写入需要可用的加密密钥" }),
            )));
        };
        let plaintext = value.as_str().unwrap();
        value = json!(encrypt_python_fernet_plaintext(encryption_key, plaintext)
            .map_err(|err| GatewayError::Internal(err.to_string()))?);
    }

    let updated = state
        .upsert_system_config_entry(&normalized_key, &value, description.as_deref())
        .await?;
    let display_value = if is_sensitive_admin_system_config_key(&normalized_key) {
        json!("********")
    } else {
        updated.value.clone()
    };
    Ok(Ok(json!({
        "key": updated.key,
        "value": display_value,
        "description": updated.description,
        "updated_at": updated.updated_at_unix_secs.and_then(unix_secs_to_rfc3339),
    })))
}

pub(crate) async fn delete_admin_system_config(
    state: &AppState,
    requested_key: &str,
) -> Result<Result<serde_json::Value, (http::StatusCode, serde_json::Value)>, GatewayError> {
    let delete_keys = admin_system_config_delete_keys(requested_key);
    let mut deleted = false;
    for key in &delete_keys {
        deleted |= state.delete_system_config_value(key).await?;
    }
    if !deleted {
        return Ok(Err((
            http::StatusCode::NOT_FOUND,
            json!({ "detail": format!("配置项 '{requested_key}' 不存在") }),
        )));
    }
    Ok(Ok(json!({
        "message": format!("配置项 '{}' 已删除", requested_key.trim()),
    })))
}
