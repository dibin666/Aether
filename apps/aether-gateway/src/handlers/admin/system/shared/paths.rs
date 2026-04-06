pub(crate) fn is_admin_management_tokens_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/management-tokens" | "/api/admin/management-tokens/"
    )
}

pub(crate) fn is_admin_system_configs_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/system/configs" | "/api/admin/system/configs/"
    )
}

pub(crate) fn is_admin_system_email_templates_root(request_path: &str) -> bool {
    matches!(
        request_path,
        "/api/admin/system/email/templates" | "/api/admin/system/email/templates/"
    )
}

pub(crate) fn admin_system_config_key_from_path(request_path: &str) -> Option<String> {
    let value = request_path
        .strip_prefix("/api/admin/system/configs/")?
        .trim()
        .trim_matches('/')
        .to_string();
    if value.is_empty() || value.contains('/') {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn admin_system_email_template_type_from_path(request_path: &str) -> Option<String> {
    let value = request_path
        .strip_prefix("/api/admin/system/email/templates/")?
        .trim()
        .trim_matches('/')
        .to_string();
    if value.is_empty() || value.contains('/') {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn admin_system_email_template_preview_type_from_path(
    request_path: &str,
) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/system/email/templates/")?
        .strip_suffix("/preview")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_system_email_template_reset_type_from_path(
    request_path: &str,
) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/system/email/templates/")?
        .strip_suffix("/reset")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}

pub(crate) fn admin_management_token_id_from_path(request_path: &str) -> Option<String> {
    let raw = request_path.strip_prefix("/api/admin/management-tokens/")?;
    let normalized = raw.trim().trim_matches('/');
    if normalized.is_empty() || normalized.contains('/') {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(crate) fn admin_management_token_status_id_from_path(request_path: &str) -> Option<String> {
    request_path
        .strip_prefix("/api/admin/management-tokens/")?
        .strip_suffix("/status")
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty() && !value.contains('/'))
}
