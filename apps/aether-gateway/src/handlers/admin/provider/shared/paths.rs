pub(crate) fn admin_provider_id_for_keys(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/endpoints/providers/")?
        .strip_suffix("/keys")
        .map(ToOwned::to_owned)
}

pub(crate) fn is_admin_provider_ops_architectures_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/provider-ops/architectures" | "/api/admin/provider-ops/architectures/"
    )
}

pub(crate) fn admin_provider_ops_architecture_id_from_path(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/provider-ops/architectures/")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_provider_id_for_provider_ops_status(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/status")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_config(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/config")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_disconnect(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/disconnect")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_connect(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/connect")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_verify(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/verify")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_balance(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/balance")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_ops_checkin(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-ops/providers/")?
        .strip_suffix("/checkin")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn is_admin_provider_strategy_strategies_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/provider-strategy/strategies" | "/api/admin/provider-strategy/strategies/"
    )
}

pub(crate) fn admin_provider_id_for_provider_strategy_billing(
    request_path: &str,
) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-strategy/providers/")?
        .strip_suffix("/billing")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_strategy_stats(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-strategy/providers/")?
        .strip_suffix("/stats")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_id_for_provider_strategy_quota(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-strategy/providers/")?
        .strip_suffix("/quota")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_provider_ops_action_route_parts(
    request_path: &str,
) -> Option<(String, String)> {
    let raw = request_path.strip_prefix("/api/admin/provider-ops/providers/")?;
    let (provider_id, action_type) = raw.split_once("/actions/")?;
    let provider_id = provider_id.trim().trim_matches('/');
    let action_type = action_type.trim().trim_matches('/');
    if provider_id.is_empty()
        || action_type.is_empty()
        || provider_id.contains('/')
        || action_type.contains('/')
    {
        None
    } else {
        Some((provider_id.to_string(), action_type.to_string()))
    }
}

pub(crate) fn is_admin_provider_ops_batch_balance_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/provider-ops/batch/balance" | "/api/admin/provider-ops/batch/balance/"
    )
}

pub(crate) fn admin_provider_id_for_refresh_quota(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/endpoints/providers/")?
        .strip_suffix("/refresh-quota")
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_id_for_health_monitor(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let raw = raw.strip_suffix("/health-monitor")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_provider_id_for_mapping_preview(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let raw = raw.strip_suffix("/mapping-preview")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_provider_id_for_pool_status(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/pool-status")
}

pub(crate) fn admin_provider_pool_key_route_parts(
    request_path: &str,
    marker: &str,
) -> Option<(String, String)> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let (provider_id, key_id) = raw.split_once(marker)?;
    let provider_id = provider_id.trim().trim_matches('/');
    let key_id = key_id.trim().trim_matches('/');
    if provider_id.is_empty()
        || key_id.is_empty()
        || provider_id.contains('/')
        || key_id.contains('/')
    {
        None
    } else {
        Some((provider_id.to_string(), key_id.to_string()))
    }
}

pub(crate) fn admin_provider_clear_pool_cooldown_parts(
    request_path: &str,
) -> Option<(String, String)> {
    admin_provider_pool_key_route_parts(request_path, "/pool/clear-cooldown/")
}

pub(crate) fn admin_provider_reset_pool_cost_parts(request_path: &str) -> Option<(String, String)> {
    admin_provider_pool_key_route_parts(request_path, "/pool/reset-cost/")
}

pub(crate) fn admin_provider_delete_task_parts(request_path: &str) -> Option<(String, String)> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let (provider_id, task_id) = raw.split_once("/delete-task/")?;
    let provider_id = provider_id.trim().trim_matches('/');
    let task_id = task_id.trim().trim_matches('/');
    if provider_id.is_empty()
        || task_id.is_empty()
        || provider_id.contains('/')
        || task_id.contains('/')
    {
        None
    } else {
        Some((provider_id.to_string(), task_id.to_string()))
    }
}

pub(crate) fn admin_provider_id_for_summary(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let raw = raw.strip_suffix("/summary")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_provider_id_for_manage_path(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn is_admin_providers_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/providers" | "/api/admin/providers/"
    )
}

pub(crate) fn admin_provider_id_for_models_list(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/models")
}

pub(crate) fn admin_provider_id_for_suffix(request_path: &str, suffix: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let raw = raw.strip_suffix(suffix)?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_provider_model_route_parts(request_path: &str) -> Option<(String, String)> {
    let raw = request_path.strip_prefix("/api/admin/providers/")?;
    let (provider_id, model_id) = raw.split_once("/models/")?;
    let provider_id = provider_id.trim().trim_matches('/');
    let model_id = model_id.trim().trim_matches('/');
    if provider_id.is_empty()
        || model_id.is_empty()
        || provider_id.contains('/')
        || model_id.contains('/')
    {
        None
    } else {
        Some((provider_id.to_string(), model_id.to_string()))
    }
}

pub(crate) fn admin_provider_models_batch_path(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/models/batch")
}

pub(crate) fn admin_provider_available_source_models_path(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/available-source-models")
}

pub(crate) fn admin_provider_assign_global_models_path(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/assign-global-models")
}

pub(crate) fn admin_provider_import_models_path(request_path: &str) -> Option<String> {
    admin_provider_id_for_suffix(request_path, "/import-from-upstream")
}

pub(crate) fn admin_reveal_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/endpoints/keys/")?
        .strip_suffix("/reveal")
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_export_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/endpoints/keys/")?
        .strip_suffix("/export")
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_clear_oauth_invalid_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/endpoints/keys/")?
        .strip_suffix("/clear-oauth-invalid")
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_update_key_id(request_path: &str) -> Option<String> {
    let key_id = request_path.strip_prefix("/api/admin/endpoints/keys/")?;
    (!key_id.is_empty() && !key_id.contains('/')).then_some(key_id.to_string())
}

pub(crate) fn admin_provider_oauth_start_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/keys/")?
        .strip_suffix("/start")
        .filter(|key_id| !key_id.is_empty() && !key_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_start_provider_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/start")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_complete_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/keys/")?
        .strip_suffix("/complete")
        .filter(|key_id| !key_id.is_empty() && !key_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_refresh_key_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/keys/")?
        .strip_suffix("/refresh")
        .filter(|key_id| !key_id.is_empty() && !key_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_complete_provider_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/complete")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_import_provider_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/import-refresh-token")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_batch_import_provider_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/batch-import")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_batch_import_task_provider_id(
    request_path: &str,
) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/batch-import/tasks")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_batch_import_task_path(
    request_path: &str,
) -> Option<(String, String)> {
    let suffix = request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/")
        .unwrap_or(request_path.strip_prefix("/api/admin/provider-oauth/providers/")?);
    let (provider_id, task_path) = suffix.split_once("/batch-import/tasks/")?;
    if provider_id.is_empty()
        || provider_id.contains('/')
        || task_path.is_empty()
        || task_path.contains('/')
    {
        return None;
    }
    Some((provider_id.to_string(), task_path.to_string()))
}

pub(crate) fn admin_provider_oauth_device_authorize_provider_id(
    request_path: &str,
) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/device-authorize")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}

pub(crate) fn admin_provider_oauth_device_poll_provider_id(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/provider-oauth/providers/")?
        .strip_suffix("/device-poll")
        .filter(|provider_id| !provider_id.is_empty() && !provider_id.contains('/'))
        .map(ToOwned::to_owned)
}
