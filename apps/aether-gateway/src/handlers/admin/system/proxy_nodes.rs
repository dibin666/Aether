use std::time::{Duration, Instant};

use crate::handlers::admin::request::{AdminAppState, AdminRequestContext};
use crate::handlers::admin::shared::query_param_value;
use crate::maintenance::{
    cancel_proxy_upgrade_rollout, clear_proxy_upgrade_rollout_conflicts,
    restore_proxy_upgrade_rollout_skipped_nodes, retry_proxy_upgrade_rollout_node,
    skip_proxy_upgrade_rollout_node, start_proxy_upgrade_rollout, ProxyUpgradeRolloutProbeConfig,
};
use crate::GatewayError;
use aether_admin::system::{
    admin_proxy_node_event_node_id_from_path, build_admin_proxy_node_payload,
    build_admin_proxy_nodes_data_unavailable_response, build_admin_proxy_nodes_not_found_response,
};
use axum::{
    body::{Body, Bytes},
    http,
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{net::TcpStream, time::timeout};

#[derive(Debug, Deserialize)]
struct ProxyNodeRegisterRequest {
    name: String,
    ip: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    heartbeat_interval: Option<i32>,
    #[serde(default)]
    active_connections: Option<i32>,
    #[serde(default)]
    total_requests: Option<i64>,
    #[serde(default)]
    avg_latency_ms: Option<f64>,
    #[serde(default)]
    hardware_info: Option<Value>,
    #[serde(default)]
    estimated_max_concurrency: Option<i32>,
    #[serde(default)]
    proxy_metadata: Option<Value>,
    #[serde(default)]
    proxy_version: Option<String>,
    #[serde(default)]
    tunnel_mode: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ProxyNodeHeartbeatRequest {
    node_id: String,
    #[serde(default)]
    heartbeat_interval: Option<i32>,
    #[serde(default)]
    active_connections: Option<i32>,
    #[serde(default)]
    total_requests: Option<i64>,
    #[serde(default)]
    avg_latency_ms: Option<f64>,
    #[serde(default)]
    failed_requests: Option<i64>,
    #[serde(default)]
    dns_failures: Option<i64>,
    #[serde(default)]
    stream_errors: Option<i64>,
    #[serde(default)]
    proxy_metadata: Option<Value>,
    #[serde(default)]
    proxy_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProxyNodeUnregisterRequest {
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct ManualProxyNodeCreateRequest {
    name: String,
    proxy_url: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    region: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManualProxyNodeUpdateRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    proxy_url: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    region: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProxyNodeTestUrlRequest {
    proxy_url: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProxyNodeBatchUpgradeRequest {
    version: String,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    cooldown_secs: Option<u64>,
    #[serde(default)]
    probe_url: Option<String>,
    #[serde(default)]
    probe_timeout_secs: Option<u64>,
}

const JSON_OBJECT_REQUIRED_DETAIL: &str = "请求体必须是合法的 JSON 对象";
const DEFAULT_PROXY_UPGRADE_BATCH_SIZE: usize = 1;
const DEFAULT_PROXY_UPGRADE_COOLDOWN_SECS: u64 = 60;
const DEFAULT_PROXY_UPGRADE_PROBE_TIMEOUT_SECS: u64 = 10;

pub(crate) async fn maybe_build_local_admin_proxy_nodes_response(
    state: &AdminAppState<'_>,
    request_context: &AdminRequestContext<'_>,
    request_body: Option<&Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(decision) = request_context.decision() else {
        return Ok(None);
    };

    if decision.route_family.as_deref() != Some("proxy_nodes_manage") {
        return Ok(None);
    }

    if decision.route_kind.as_deref() == Some("list_nodes")
        && request_context.method() == http::Method::GET
        && matches!(
            request_context.path(),
            "/api/admin/proxy-nodes" | "/api/admin/proxy-nodes/"
        )
    {
        let skip = query_param_value(request_context.query_string(), "skip")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let limit = query_param_value(request_context.query_string(), "limit")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0 && *value <= 1000)
            .unwrap_or(100);
        let status = query_param_value(request_context.query_string(), "status")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        return Ok(Some(
            state
                .build_admin_proxy_nodes_list_response(skip, limit, status)
                .await?,
        ));
    }

    if decision.route_kind.as_deref() == Some("list_node_events")
        && request_context.method() == http::Method::GET
    {
        let Some(node_id) = admin_proxy_node_event_node_id_from_path(request_context.path()) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };

        let limit = query_param_value(request_context.query_string(), "limit")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0 && *value <= 200)
            .unwrap_or(50);
        return Ok(Some(
            state
                .build_admin_proxy_node_events_response(node_id, limit)
                .await?,
        ));
    }

    if decision.route_kind.as_deref() == Some("register_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let input = match parse_json_body::<ProxyNodeRegisterRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let mutation = match validate_register_request(input, request_context) {
            Ok(mutation) => mutation,
            Err(response) => return Ok(Some(response)),
        };
        let Some(node) = state.register_proxy_node(&mutation).await? else {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        };
        return Ok(Some(
            Json(json!({
                "node_id": node.id,
                "node": build_admin_proxy_node_payload(&node),
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("heartbeat_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let input = match parse_json_body::<ProxyNodeHeartbeatRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let mutation = match validate_heartbeat_request(input) {
            Ok(mutation) => mutation,
            Err(response) => return Ok(Some(response)),
        };
        let Some(existing) = state.find_proxy_node(&mutation.node_id).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        if !existing.tunnel_mode {
            return Ok(Some(bad_request_response(
                "non-tunnel mode is no longer supported, please upgrade aether-proxy to use tunnel mode",
            )));
        }
        let Some(node) = state.apply_proxy_node_heartbeat(&mutation).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        return Ok(Some(
            Json(json!({
                "message": "heartbeat ok",
                "node": build_admin_proxy_node_payload(&node),
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("unregister_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let input = match parse_json_body::<ProxyNodeUnregisterRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let node_id = match validate_node_id(&input.node_id) {
            Ok(node_id) => node_id,
            Err(response) => return Ok(Some(response)),
        };
        let Some(node) = state.unregister_proxy_node(&node_id).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        return Ok(Some(
            Json(json!({
                "message": "unregistered",
                "node_id": node.id,
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("create_manual_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let input = match parse_json_body::<ManualProxyNodeCreateRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let mutation = match validate_manual_create_request(input, request_context) {
            Ok(mutation) => mutation,
            Err(response) => return Ok(Some(response)),
        };
        let Some(node) = state.create_manual_proxy_node(&mutation).await? else {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        };
        return Ok(Some(
            Json(json!({
                "node_id": node.id,
                "node": build_admin_proxy_node_payload(&node),
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("update_manual_node")
        && request_context.method() == http::Method::PATCH
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_node_id_from_path(request_context.path()) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        let input = match parse_json_body::<ManualProxyNodeUpdateRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let mutation = match validate_manual_update_request(node_id, input) {
            Ok(mutation) => mutation,
            Err(response) => return Ok(Some(response)),
        };
        let Some(node) = state.update_manual_proxy_node(&mutation).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        return Ok(Some(
            Json(json!({
                "node_id": node.id,
                "node": build_admin_proxy_node_payload(&node),
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("delete_node")
        && request_context.method() == http::Method::DELETE
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_node_id_from_path(request_context.path()) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        let Some(_deleted_node) = state.delete_proxy_node(&node_id).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        let cleanup = clear_deleted_proxy_node_references(state, &node_id).await?;
        return Ok(Some(
            Json(json!({
                "message": build_delete_proxy_node_message(&cleanup),
                "node_id": node_id,
                "cleared_system_proxy": cleanup.cleared_system_proxy,
                "cleared_providers": cleanup.cleared_providers,
                "cleared_endpoints": cleanup.cleared_endpoints,
                "cleared_keys": cleanup.cleared_keys,
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("test_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_test_node_id_from_path(request_context.path()) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        let Some(node) = state.find_proxy_node(&node_id).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        return Ok(Some(
            Json(test_proxy_node_connectivity(&node).await).into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("test_proxy_url")
        && request_context.method() == http::Method::POST
    {
        let input = match parse_json_body::<ProxyNodeTestUrlRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let normalized = match validate_proxy_test_url_request(input) {
            Ok(normalized) => normalized,
            Err(response) => return Ok(Some(response)),
        };
        return Ok(Some(
            Json(
                test_manual_proxy_connectivity(
                    &normalized.proxy_url,
                    normalized.host.as_str(),
                    normalized.port,
                )
                .await,
            )
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("update_node_config")
        && request_context.method() == http::Method::PUT
    {
        if !state.has_proxy_node_writer() {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_config_node_id_from_path(request_context.path())
        else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        let raw = match parse_json_object_body(request_body) {
            Ok(raw) => raw,
            Err(response) => return Ok(Some(response)),
        };
        let Some(existing) = state.find_proxy_node(&node_id).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        if existing.is_manual {
            return Ok(Some(bad_request_response("手动节点不支持远程配置下发")));
        }
        let mutation = match validate_remote_config_request(node_id, &raw) {
            Ok(mutation) => mutation,
            Err(response) => return Ok(Some(response)),
        };
        let Some(node) = state.update_proxy_node_remote_config(&mutation).await? else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };
        return Ok(Some(
            Json(json!({
                "node_id": node.id,
                "config_version": node.config_version,
                "remote_config": node.remote_config,
                "node": build_admin_proxy_node_payload(&node),
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("batch_upgrade_nodes")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let input = match parse_json_body::<ProxyNodeBatchUpgradeRequest>(request_body) {
            Ok(input) => input,
            Err(response) => return Ok(Some(response)),
        };
        let version = match validate_version(&input.version) {
            Ok(version) => version,
            Err(response) => return Ok(Some(response)),
        };
        let batch_size = match validate_batch_size(input.batch_size) {
            Ok(batch_size) => batch_size,
            Err(response) => return Ok(Some(response)),
        };
        let cooldown_secs = match validate_cooldown_secs(input.cooldown_secs) {
            Ok(cooldown_secs) => cooldown_secs,
            Err(response) => return Ok(Some(response)),
        };
        let probe =
            match validate_probe_config(input.probe_url.as_deref(), input.probe_timeout_secs) {
                Ok(probe) => probe,
                Err(response) => return Ok(Some(response)),
            };
        let rollout = start_proxy_upgrade_rollout(
            &state.app().data,
            version.clone(),
            batch_size,
            cooldown_secs,
            probe,
        )
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))?;

        return Ok(Some(
            Json(json!({
                "version": version,
                "batch_size": rollout.batch_size,
                "cooldown_secs": rollout.cooldown_secs,
                "updated": rollout.updated,
                "skipped": rollout.skipped,
                "node_ids": rollout.node_ids,
                "blocked": rollout.blocked,
                "pending_node_ids": rollout.pending_node_ids,
                "rollout_active": rollout.rollout_active,
                "completed": rollout.completed,
                "remaining": rollout.remaining,
            }))
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("cancel_upgrade_rollout")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }

        let summary = cancel_proxy_upgrade_rollout(&state.app().data)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(
            Json(match summary {
                Some(summary) => json!({
                    "cancelled": true,
                    "version": summary.version,
                    "pending_node_ids": summary.pending_node_ids,
                    "conflict_node_ids": summary.conflict_node_ids,
                    "completed": summary.completed,
                    "remaining": summary.remaining,
                }),
                None => json!({
                    "cancelled": false,
                    "rollout_active": false,
                }),
            })
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("clear_upgrade_rollout_conflicts")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }

        let summary = clear_proxy_upgrade_rollout_conflicts(&state.app().data)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(
            Json(match summary {
                Some(summary) => json!({
                    "version": summary.version,
                    "cleared": summary.cleared_node_ids.len(),
                    "node_ids": summary.cleared_node_ids,
                    "updated": summary.updated,
                    "blocked": summary.blocked,
                    "pending_node_ids": summary.pending_node_ids,
                    "rollout_active": summary.rollout_active,
                    "completed": summary.completed,
                    "remaining": summary.remaining,
                }),
                None => json!({
                    "version": null,
                    "cleared": 0,
                    "node_ids": [],
                    "updated": 0,
                    "blocked": false,
                    "pending_node_ids": [],
                    "rollout_active": false,
                    "completed": 0,
                    "remaining": 0,
                }),
            })
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("restore_skipped_upgrade_rollout_nodes")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }

        let summary = restore_proxy_upgrade_rollout_skipped_nodes(&state.app().data)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(
            Json(match summary {
                Some(summary) => json!({
                    "version": summary.version,
                    "restored": summary.restored_node_ids.len(),
                    "node_ids": summary.restored_node_ids,
                    "skipped_node_ids": summary.skipped_node_ids,
                    "updated": summary.updated,
                    "blocked": summary.blocked,
                    "pending_node_ids": summary.pending_node_ids,
                    "rollout_active": summary.rollout_active,
                    "completed": summary.completed,
                    "remaining": summary.remaining,
                }),
                None => json!({
                    "version": null,
                    "restored": 0,
                    "node_ids": [],
                    "skipped_node_ids": [],
                    "updated": 0,
                    "blocked": false,
                    "pending_node_ids": [],
                    "rollout_active": false,
                    "completed": 0,
                    "remaining": 0,
                }),
            })
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("skip_upgrade_rollout_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_upgrade_action_node_id_from_path(
            request_context.path(),
            "/upgrade/skip",
        ) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };

        let summary = skip_proxy_upgrade_rollout_node(&state.app().data, &node_id)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(
            Json(match summary {
                Some(summary) => json!({
                    "version": summary.version,
                    "node_id": summary.node_id,
                    "skipped_node_ids": summary.skipped_node_ids,
                    "updated": summary.updated,
                    "blocked": summary.blocked,
                    "pending_node_ids": summary.pending_node_ids,
                    "rollout_active": summary.rollout_active,
                    "completed": summary.completed,
                    "remaining": summary.remaining,
                }),
                None => json!({
                    "version": null,
                    "node_id": node_id,
                    "skipped_node_ids": [],
                    "updated": 0,
                    "blocked": false,
                    "pending_node_ids": [],
                    "rollout_active": false,
                    "completed": 0,
                    "remaining": 0,
                }),
            })
            .into_response(),
        ));
    }

    if decision.route_kind.as_deref() == Some("retry_upgrade_rollout_node")
        && request_context.method() == http::Method::POST
    {
        if !state.has_proxy_node_reader()
            || !state.has_proxy_node_writer()
            || !state.app().data.has_system_config_store()
        {
            return Ok(Some(build_admin_proxy_nodes_data_unavailable_response()));
        }
        let Some(node_id) = admin_proxy_node_upgrade_action_node_id_from_path(
            request_context.path(),
            "/upgrade/retry",
        ) else {
            return Ok(Some(build_admin_proxy_nodes_not_found_response()));
        };

        let summary = retry_proxy_upgrade_rollout_node(&state.app().data, &node_id)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        return Ok(Some(
            Json(match summary {
                Some(summary) => json!({
                    "version": summary.version,
                    "node_id": summary.node_id,
                    "skipped_node_ids": summary.skipped_node_ids,
                    "updated": summary.updated,
                    "blocked": summary.blocked,
                    "pending_node_ids": summary.pending_node_ids,
                    "rollout_active": summary.rollout_active,
                    "completed": summary.completed,
                    "remaining": summary.remaining,
                }),
                None => json!({
                    "version": null,
                    "node_id": node_id,
                    "skipped_node_ids": [],
                    "updated": 0,
                    "blocked": false,
                    "pending_node_ids": [],
                    "rollout_active": false,
                    "completed": 0,
                    "remaining": 0,
                }),
            })
            .into_response(),
        ));
    }

    Ok(Some(build_admin_proxy_nodes_data_unavailable_response()))
}

#[derive(Debug, Default)]
struct DeletedProxyNodeCleanup {
    cleared_system_proxy: bool,
    cleared_providers: usize,
    cleared_endpoints: usize,
    cleared_keys: usize,
}

#[derive(Debug, Clone)]
struct NormalizedManualProxyEndpoint {
    proxy_url: String,
    host: String,
    port: u16,
    node_ip: String,
    node_port: i32,
}

async fn clear_deleted_proxy_node_references(
    state: &AdminAppState<'_>,
    node_id: &str,
) -> Result<DeletedProxyNodeCleanup, GatewayError> {
    let mut cleanup = DeletedProxyNodeCleanup::default();

    if state.app().data.has_system_config_store() {
        let is_system_proxy = state
            .read_system_config_json_value("system_proxy_node_id")
            .await?
            .and_then(|value| value.as_str().map(str::trim).map(ToOwned::to_owned))
            .is_some_and(|value| value == node_id);
        if is_system_proxy {
            state
                .upsert_system_config_json_value(
                    "system_proxy_node_id",
                    &serde_json::Value::Null,
                    None,
                )
                .await?;
            cleanup.cleared_system_proxy = true;
        }
    }

    if state.app().has_provider_catalog_data_reader()
        && state.app().has_provider_catalog_data_writer()
    {
        let providers = state.list_provider_catalog_providers(false).await?;
        let provider_ids = providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect::<Vec<_>>();

        for mut provider in providers {
            if !proxy_reference_matches_node_id(provider.proxy.as_ref(), node_id) {
                continue;
            }
            provider.proxy = None;
            if state
                .update_provider_catalog_provider(&provider)
                .await?
                .is_some()
            {
                cleanup.cleared_providers = cleanup.cleared_providers.saturating_add(1);
            }
        }

        if !provider_ids.is_empty() {
            let endpoints = state
                .list_provider_catalog_endpoints_by_provider_ids(&provider_ids)
                .await?;
            for mut endpoint in endpoints {
                if !proxy_reference_matches_node_id(endpoint.proxy.as_ref(), node_id) {
                    continue;
                }
                endpoint.proxy = None;
                if state
                    .update_provider_catalog_endpoint(&endpoint)
                    .await?
                    .is_some()
                {
                    cleanup.cleared_endpoints = cleanup.cleared_endpoints.saturating_add(1);
                }
            }

            let keys = state
                .list_provider_catalog_keys_by_provider_ids(&provider_ids)
                .await?;
            for mut key in keys {
                if !proxy_reference_matches_node_id(key.proxy.as_ref(), node_id) {
                    continue;
                }
                key.proxy = None;
                if state.update_provider_catalog_key(&key).await?.is_some() {
                    cleanup.cleared_keys = cleanup.cleared_keys.saturating_add(1);
                }
            }
        }
    }

    Ok(cleanup)
}

fn build_delete_proxy_node_message(cleanup: &DeletedProxyNodeCleanup) -> String {
    let mut parts = vec!["deleted".to_string()];
    if cleanup.cleared_system_proxy {
        parts.push("system default proxy cleared".to_string());
    }
    if cleanup.cleared_providers > 0 || cleanup.cleared_endpoints > 0 || cleanup.cleared_keys > 0 {
        parts.push(format!(
            "cleared proxy refs from {} provider(s), {} endpoint(s), {} key(s)",
            cleanup.cleared_providers, cleanup.cleared_endpoints, cleanup.cleared_keys
        ));
    }
    parts.join(", ")
}

fn proxy_reference_matches_node_id(value: Option<&Value>, node_id: &str) -> bool {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("node_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value == node_id)
}

async fn test_proxy_node_connectivity(
    node: &aether_data::repository::proxy_nodes::StoredProxyNode,
) -> Value {
    if node.is_manual {
        let Some(proxy_url) = node.proxy_url.as_deref() else {
            return json!({
                "success": false,
                "latency_ms": null,
                "exit_ip": null,
                "error": "手动节点缺少 proxy_url",
            });
        };
        let endpoint = match parse_manual_proxy_endpoint(proxy_url, "proxy_url") {
            Ok(endpoint) => endpoint,
            Err(detail) => {
                return json!({
                    "success": false,
                    "latency_ms": null,
                    "exit_ip": null,
                    "error": detail,
                });
            }
        };
        return test_manual_proxy_connectivity(
            &endpoint.proxy_url,
            endpoint.host.as_str(),
            endpoint.port,
        )
        .await;
    }

    if !node.tunnel_mode {
        return json!({
            "success": false,
            "latency_ms": null,
            "exit_ip": null,
            "error": "non-tunnel mode is no longer supported, please upgrade aether-proxy to use tunnel mode",
        });
    }

    if !node.status.eq_ignore_ascii_case("online") || !node.tunnel_connected {
        return json!({
            "success": false,
            "latency_ms": null,
            "exit_ip": null,
            "error": "tunnel 未连接",
        });
    }

    json!({
        "success": true,
        "latency_ms": node.avg_latency_ms.map(|value| value.max(0.0).round() as u64),
        "exit_ip": null,
        "error": null,
    })
}

async fn test_manual_proxy_connectivity(_proxy_url: &str, host: &str, port: u16) -> Value {
    let started_at = Instant::now();
    match timeout(Duration::from_secs(5), TcpStream::connect((host, port))).await {
        Ok(Ok(stream)) => {
            drop(stream);
            json!({
                "success": true,
                "latency_ms": started_at.elapsed().as_millis() as u64,
                "exit_ip": null,
                "error": null,
            })
        }
        Ok(Err(error)) => json!({
            "success": false,
            "latency_ms": null,
            "exit_ip": null,
            "error": sanitize_proxy_error(&error.to_string()),
        }),
        Err(_) => json!({
            "success": false,
            "latency_ms": null,
            "exit_ip": null,
            "error": "连接超时",
        }),
    }
}

fn validate_register_request(
    input: ProxyNodeRegisterRequest,
    request_context: &AdminRequestContext<'_>,
) -> Result<aether_data::repository::proxy_nodes::ProxyNodeRegistrationMutation, Response<Body>> {
    let name = normalize_required_string(&input.name, "name", 100)?;
    let ip = normalize_ip_address(&input.ip)?;
    let heartbeat_interval = validate_optional_i32_range(
        input.heartbeat_interval.unwrap_or(30),
        "heartbeat_interval",
        5,
        600,
    )?;
    if !input.tunnel_mode.unwrap_or(true) {
        return Err(bad_request_response("仅支持 tunnel_mode=true"));
    }
    validate_optional_counter(
        input.active_connections.map(i64::from),
        "active_connections",
    )?;
    validate_optional_counter(input.total_requests, "total_requests")?;
    validate_optional_counter(
        input.estimated_max_concurrency.map(i64::from),
        "estimated_max_concurrency",
    )?;
    if input
        .avg_latency_ms
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(bad_request_response("avg_latency_ms 必须是非负有限数值"));
    }
    validate_optional_object(input.hardware_info.as_ref(), "hardware_info")?;
    validate_optional_object(input.proxy_metadata.as_ref(), "proxy_metadata")?;

    let registered_by = request_context
        .decision()
        .and_then(|decision| decision.admin_principal.as_ref())
        .map(|principal| principal.user_id.clone());

    Ok(
        aether_data::repository::proxy_nodes::ProxyNodeRegistrationMutation {
            name,
            ip,
            port: i32::from(input.port.unwrap_or_default()),
            region: normalize_optional_string(input.region.as_deref(), "region", 100)?,
            heartbeat_interval,
            active_connections: input.active_connections,
            total_requests: input.total_requests,
            avg_latency_ms: input.avg_latency_ms,
            hardware_info: input.hardware_info,
            estimated_max_concurrency: input.estimated_max_concurrency,
            proxy_metadata: input.proxy_metadata,
            proxy_version: normalize_optional_string(
                input.proxy_version.as_deref(),
                "proxy_version",
                20,
            )?,
            registered_by,
            tunnel_mode: true,
        },
    )
}

fn validate_manual_create_request(
    input: ManualProxyNodeCreateRequest,
    request_context: &AdminRequestContext<'_>,
) -> Result<aether_data::repository::proxy_nodes::ProxyNodeManualCreateMutation, Response<Body>> {
    let endpoint = normalize_manual_proxy_endpoint(&input.proxy_url)?;
    let registered_by = request_context
        .decision()
        .and_then(|decision| decision.admin_principal.as_ref())
        .map(|principal| principal.user_id.clone());

    Ok(
        aether_data::repository::proxy_nodes::ProxyNodeManualCreateMutation {
            name: normalize_required_string(&input.name, "name", 100)?,
            ip: endpoint.node_ip,
            port: endpoint.node_port,
            region: normalize_optional_string(input.region.as_deref(), "region", 100)?,
            proxy_url: endpoint.proxy_url,
            proxy_username: normalize_optional_string(input.username.as_deref(), "username", 255)?,
            proxy_password: normalize_optional_string(input.password.as_deref(), "password", 500)?,
            registered_by,
        },
    )
}

fn validate_manual_update_request(
    node_id: String,
    input: ManualProxyNodeUpdateRequest,
) -> Result<aether_data::repository::proxy_nodes::ProxyNodeManualUpdateMutation, Response<Body>> {
    let endpoint = match input.proxy_url.as_deref() {
        Some(proxy_url) => Some(normalize_manual_proxy_endpoint(proxy_url)?),
        None => None,
    };
    let name = normalize_optional_string(input.name.as_deref(), "name", 100)?;
    let region = normalize_optional_string(input.region.as_deref(), "region", 100)?;
    let proxy_username = normalize_optional_string(input.username.as_deref(), "username", 255)?;
    let proxy_password = normalize_optional_string(input.password.as_deref(), "password", 500)?;

    if name.is_none()
        && region.is_none()
        && proxy_username.is_none()
        && proxy_password.is_none()
        && endpoint.is_none()
    {
        return Err(bad_request_response("至少提供一个可更新字段"));
    }

    Ok(
        aether_data::repository::proxy_nodes::ProxyNodeManualUpdateMutation {
            node_id,
            name,
            ip: endpoint.as_ref().map(|value| value.node_ip.clone()),
            port: endpoint.as_ref().map(|value| value.node_port),
            region,
            proxy_url: endpoint.map(|value| value.proxy_url),
            proxy_username,
            proxy_password,
        },
    )
}

fn validate_proxy_test_url_request(
    input: ProxyNodeTestUrlRequest,
) -> Result<NormalizedManualProxyEndpoint, Response<Body>> {
    let _ = normalize_optional_string(input.username.as_deref(), "username", 255)?;
    let _ = normalize_optional_string(input.password.as_deref(), "password", 500)?;
    normalize_manual_proxy_endpoint(&input.proxy_url)
}

fn admin_proxy_node_upgrade_action_node_id_from_path(path: &str, suffix: &str) -> Option<String> {
    let normalized = path.trim_end_matches('/');
    let node_id = normalized.strip_prefix("/api/admin/proxy-nodes/")?;
    let node_id = node_id.strip_suffix(suffix)?;
    if node_id.is_empty() || node_id.contains('/') {
        None
    } else {
        Some(node_id.to_string())
    }
}

fn admin_proxy_node_node_id_from_path(path: &str) -> Option<String> {
    let normalized = path.trim_end_matches('/');
    let node_id = normalized.strip_prefix("/api/admin/proxy-nodes/")?;
    if node_id.is_empty() || node_id.contains('/') {
        None
    } else {
        Some(node_id.to_string())
    }
}

fn admin_proxy_node_test_node_id_from_path(path: &str) -> Option<String> {
    let normalized = path.trim_end_matches('/');
    let node_id = normalized.strip_prefix("/api/admin/proxy-nodes/")?;
    let node_id = node_id.strip_suffix("/test")?;
    if node_id.is_empty() || node_id.contains('/') {
        None
    } else {
        Some(node_id.to_string())
    }
}

fn validate_batch_size(batch_size: Option<usize>) -> Result<usize, Response<Body>> {
    let batch_size = batch_size.unwrap_or(DEFAULT_PROXY_UPGRADE_BATCH_SIZE);
    if (1..=100).contains(&batch_size) {
        Ok(batch_size)
    } else {
        Err(bad_request_response("batch_size 必须在 1 到 100 之间"))
    }
}

fn validate_cooldown_secs(cooldown_secs: Option<u64>) -> Result<u64, Response<Body>> {
    let cooldown_secs = cooldown_secs.unwrap_or(DEFAULT_PROXY_UPGRADE_COOLDOWN_SECS);
    if cooldown_secs <= 3600 {
        Ok(cooldown_secs)
    } else {
        Err(bad_request_response("cooldown_secs 不能超过 3600"))
    }
}

fn validate_probe_config(
    probe_url: Option<&str>,
    probe_timeout_secs: Option<u64>,
) -> Result<Option<ProxyUpgradeRolloutProbeConfig>, Response<Body>> {
    let Some(probe_url) = probe_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = reqwest::Url::parse(probe_url)
        .map_err(|_| bad_request_response("probe_url 必须是合法的 http/https URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(bad_request_response("probe_url 仅支持 http 或 https"));
    }
    if parsed.as_str().len() > 2048 {
        return Err(bad_request_response("probe_url 长度不能超过 2048"));
    }
    let timeout_secs = probe_timeout_secs.unwrap_or(DEFAULT_PROXY_UPGRADE_PROBE_TIMEOUT_SECS);
    if !(5..=60).contains(&timeout_secs) {
        return Err(bad_request_response(
            "probe_timeout_secs 必须在 5 到 60 秒之间",
        ));
    }
    Ok(Some(ProxyUpgradeRolloutProbeConfig {
        url: parsed.to_string(),
        timeout_secs,
    }))
}

fn validate_heartbeat_request(
    input: ProxyNodeHeartbeatRequest,
) -> Result<aether_data::repository::proxy_nodes::ProxyNodeHeartbeatMutation, Response<Body>> {
    let node_id = validate_node_id(&input.node_id)?;
    if let Some(interval) = input.heartbeat_interval {
        validate_optional_i32_range(interval, "heartbeat_interval", 5, 600)?;
    }
    validate_optional_counter(
        input.active_connections.map(i64::from),
        "active_connections",
    )?;
    validate_optional_counter(input.total_requests, "total_requests")?;
    validate_optional_counter(input.failed_requests, "failed_requests")?;
    validate_optional_counter(input.dns_failures, "dns_failures")?;
    validate_optional_counter(input.stream_errors, "stream_errors")?;
    if input
        .avg_latency_ms
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(bad_request_response("avg_latency_ms 必须是非负有限数值"));
    }
    validate_optional_object(input.proxy_metadata.as_ref(), "proxy_metadata")?;

    Ok(
        aether_data::repository::proxy_nodes::ProxyNodeHeartbeatMutation {
            node_id,
            heartbeat_interval: input.heartbeat_interval,
            active_connections: input.active_connections,
            total_requests_delta: input.total_requests,
            avg_latency_ms: input.avg_latency_ms,
            failed_requests_delta: input.failed_requests,
            dns_failures_delta: input.dns_failures,
            stream_errors_delta: input.stream_errors,
            proxy_metadata: input.proxy_metadata,
            proxy_version: normalize_optional_string(
                input.proxy_version.as_deref(),
                "proxy_version",
                20,
            )?,
        },
    )
}

fn validate_remote_config_request(
    node_id: String,
    raw: &serde_json::Map<String, Value>,
) -> Result<aether_data::repository::proxy_nodes::ProxyNodeRemoteConfigMutation, Response<Body>> {
    let node_name = match raw.get("node_name") {
        Some(Value::Null) | None => None,
        Some(Value::String(value)) => Some(normalize_required_string(value, "node_name", 100)?),
        Some(_) => return Err(bad_request_response("node_name 必须是字符串")),
    };

    let allowed_ports = match raw.get("allowed_ports") {
        Some(Value::Null) | None => None,
        Some(Value::Array(items)) => {
            let mut ports = Vec::with_capacity(items.len());
            for item in items {
                let Some(port) = item.as_u64() else {
                    return Err(bad_request_response("allowed_ports 必须是端口数字数组"));
                };
                if !(1..=65535).contains(&port) {
                    return Err(bad_request_response("allowed_ports 仅支持 1-65535"));
                }
                ports.push(port as u16);
            }
            Some(ports)
        }
        Some(_) => return Err(bad_request_response("allowed_ports 必须是端口数字数组")),
    };

    let log_level = match raw.get("log_level") {
        Some(Value::Null) | None => None,
        Some(Value::String(value)) => {
            let normalized = normalize_required_string(value, "log_level", 16)?;
            if !matches!(
                normalized.as_str(),
                "trace" | "debug" | "info" | "warn" | "error"
            ) {
                return Err(bad_request_response(
                    "log_level 必须是 trace/debug/info/warn/error 之一",
                ));
            }
            Some(normalized)
        }
        Some(_) => return Err(bad_request_response("log_level 必须是字符串")),
    };

    let heartbeat_interval = match raw.get("heartbeat_interval") {
        Some(Value::Null) | None => None,
        Some(value) => Some(validate_json_i32_range(
            value,
            "heartbeat_interval",
            5,
            600,
        )?),
    };

    let scheduling_state = if raw.contains_key("scheduling_state") {
        match raw.get("scheduling_state") {
            Some(Value::Null) | None => Some(None),
            Some(Value::String(value)) => {
                let normalized = normalize_required_string(value, "scheduling_state", 16)?;
                match normalized.as_str() {
                    "active" => Some(None),
                    "draining" | "cordoned" => Some(Some(normalized)),
                    _ => {
                        return Err(bad_request_response(
                            "scheduling_state 必须是 active/draining/cordoned 之一",
                        ));
                    }
                }
            }
            Some(_) => return Err(bad_request_response("scheduling_state 必须是字符串或 null")),
        }
    } else {
        None
    };

    let upgrade_to = if raw.contains_key("upgrade_to") {
        match raw.get("upgrade_to") {
            Some(Value::Null) | None => Some(None),
            Some(Value::String(value)) => {
                let normalized = value.trim();
                if normalized.is_empty() {
                    Some(None)
                } else {
                    Some(Some(validate_version(normalized)?))
                }
            }
            Some(_) => return Err(bad_request_response("upgrade_to 必须是字符串或 null")),
        }
    } else {
        None
    };

    Ok(
        aether_data::repository::proxy_nodes::ProxyNodeRemoteConfigMutation {
            node_id,
            node_name,
            allowed_ports,
            log_level,
            heartbeat_interval,
            scheduling_state,
            upgrade_to,
        },
    )
}

fn admin_proxy_node_config_node_id_from_path(path: &str) -> Option<String> {
    let value = path
        .strip_prefix("/api/admin/proxy-nodes/")?
        .strip_suffix("/config")?;
    if value.is_empty() || value.contains('/') {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_json_body<T: DeserializeOwned>(request_body: Option<&Bytes>) -> Result<T, Response<Body>> {
    let Some(request_body) = request_body else {
        return Err(bad_request_response("请求体不能为空"));
    };
    let raw_value = serde_json::from_slice::<Value>(request_body)
        .map_err(|_| bad_request_response(JSON_OBJECT_REQUIRED_DETAIL))?;
    serde_json::from_value::<T>(raw_value)
        .map_err(|_| bad_request_response(JSON_OBJECT_REQUIRED_DETAIL))
}

fn parse_json_object_body(
    request_body: Option<&Bytes>,
) -> Result<serde_json::Map<String, Value>, Response<Body>> {
    let Some(request_body) = request_body else {
        return Err(bad_request_response("请求体不能为空"));
    };
    let raw_value = serde_json::from_slice::<Value>(request_body)
        .map_err(|_| bad_request_response(JSON_OBJECT_REQUIRED_DETAIL))?;
    raw_value
        .as_object()
        .cloned()
        .ok_or_else(|| bad_request_response(JSON_OBJECT_REQUIRED_DETAIL))
}

fn normalize_manual_proxy_endpoint(
    proxy_url: &str,
) -> Result<NormalizedManualProxyEndpoint, Response<Body>> {
    parse_manual_proxy_endpoint(proxy_url, "proxy_url").map_err(bad_request_response)
}

fn parse_manual_proxy_endpoint(
    proxy_url: &str,
    field: &str,
) -> Result<NormalizedManualProxyEndpoint, String> {
    let proxy_url = proxy_url.trim();
    if proxy_url.is_empty() {
        return Err(format!("{field} 不能为空"));
    }
    if proxy_url.chars().count() > 500 {
        return Err(format!("{field} 长度不能超过 500"));
    }

    let parsed =
        reqwest::Url::parse(proxy_url).map_err(|_| format!("{field} 必须是合法的代理 URL"))?;
    let scheme = parsed.scheme().trim().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "socks5" | "socks5h") {
        return Err(format!("{field} 仅支持 http/https/socks5/socks5h 协议"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(format!("{field} 不应包含用户名或密码，请使用独立字段"));
    }
    let host = parsed
        .host_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} 缺少主机地址"))?
        .to_string();
    let port = parsed.port().unwrap_or(match scheme.as_str() {
        "https" => 443,
        "socks5" | "socks5h" => 1080,
        _ => 80,
    });
    let node_ip = if scheme == "http" {
        host.clone()
    } else {
        format!("{scheme}://{host}")
    };
    if node_ip.chars().count() > 255 {
        return Err("代理主机标识长度不能超过 255".to_string());
    }

    Ok(NormalizedManualProxyEndpoint {
        proxy_url: proxy_url.to_string(),
        host,
        port,
        node_ip,
        node_port: i32::from(port),
    })
}

fn validate_node_id(value: &str) -> Result<String, Response<Body>> {
    normalize_required_string(value, "node_id", 36)
}

fn validate_version(value: &str) -> Result<String, Response<Body>> {
    normalize_required_string(value, "version", 50)
}

fn normalize_required_string(
    value: &str,
    field: &str,
    max_len: usize,
) -> Result<String, Response<Body>> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(bad_request_response(format!("{field} 不能为空")));
    }
    if normalized.chars().count() > max_len {
        return Err(bad_request_response(format!(
            "{field} 长度不能超过 {max_len}"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_optional_string(
    value: Option<&str>,
    field: &str,
    max_len: usize,
) -> Result<Option<String>, Response<Body>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.chars().count() > max_len {
        return Err(bad_request_response(format!(
            "{field} 长度不能超过 {max_len}"
        )));
    }
    Ok(Some(normalized.to_string()))
}

fn normalize_ip_address(value: &str) -> Result<String, Response<Body>> {
    let normalized = value.trim();
    normalized
        .parse::<std::net::IpAddr>()
        .map(|ip| ip.to_string())
        .map_err(|_| bad_request_response("ip 必须是合法的 IPv4/IPv6 地址"))
}

fn sanitize_proxy_error(detail: &str) -> String {
    match detail.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('@') {
            Some((_, tail)) => format!("{scheme}://***@{tail}"),
            None => detail.to_string(),
        },
        None => detail.to_string(),
    }
}

fn validate_optional_counter(value: Option<i64>, field: &str) -> Result<(), Response<Body>> {
    if value.is_some_and(|value| value < 0) {
        return Err(bad_request_response(format!("{field} 必须是非负整数")));
    }
    Ok(())
}

fn validate_optional_i32_range(
    value: i32,
    field: &str,
    min: i32,
    max: i32,
) -> Result<i32, Response<Body>> {
    if !(min..=max).contains(&value) {
        return Err(bad_request_response(format!(
            "{field} 必须在 {min}-{max} 范围内"
        )));
    }
    Ok(value)
}

fn validate_json_i32_range(
    value: &Value,
    field: &str,
    min: i32,
    max: i32,
) -> Result<i32, Response<Body>> {
    let Some(raw) = value.as_i64() else {
        return Err(bad_request_response(format!("{field} 必须是整数")));
    };
    let parsed =
        i32::try_from(raw).map_err(|_| bad_request_response(format!("{field} 超出范围")))?;
    validate_optional_i32_range(parsed, field, min, max)
}

fn validate_optional_object(value: Option<&Value>, field: &str) -> Result<(), Response<Body>> {
    if value.is_some_and(|value| !value.is_object()) {
        return Err(bad_request_response(format!("{field} 必须是 JSON 对象")));
    }
    Ok(())
}

fn bad_request_response(detail: impl Into<String>) -> Response<Body> {
    (
        http::StatusCode::BAD_REQUEST,
        Json(json!({ "detail": detail.into() })),
    )
        .into_response()
}
