use crate::handlers::admin::shared::{normalize_json_object, normalize_string_list};

pub(crate) fn normalize_provider_type_input(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "custom" | "claude_code" | "kiro" | "codex" | "gemini_cli" | "antigravity"
        | "vertex_ai" => Ok(normalized),
        _ => Err(
            "provider_type 仅支持 custom / claude_code / kiro / codex / gemini_cli / antigravity / vertex_ai"
                .to_string(),
        ),
    }
}

pub(crate) fn normalize_provider_billing_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "monthly_quota" | "pay_as_you_go" | "free_tier" => Ok(normalized),
        _ => Err("billing_type 仅支持 monthly_quota / pay_as_you_go / free_tier".to_string()),
    }
}

pub(crate) fn parse_optional_rfc3339_unix_secs(
    value: &str,
    field_name: &str,
) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} 不能为空"));
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| format!("{field_name} 必须是合法的 RFC3339 时间"))?;
    u64::try_from(parsed.timestamp()).map_err(|_| format!("{field_name} 超出有效时间范围"))
}

pub(crate) fn normalize_auth_type(value: Option<&str>) -> Result<String, String> {
    let auth_type = value.unwrap_or("api_key").trim().to_ascii_lowercase();
    match auth_type.as_str() {
        "api_key" | "service_account" | "oauth" => Ok(auth_type),
        _ => Err("auth_type 仅支持 api_key / service_account / oauth".to_string()),
    }
}

pub(crate) fn validate_vertex_api_formats(
    provider_type: &str,
    auth_type: &str,
    api_formats: &[String],
) -> Result<(), String> {
    if !provider_type.trim().eq_ignore_ascii_case("vertex_ai") {
        return Ok(());
    }

    let allowed = match auth_type {
        "api_key" => &["gemini:chat"][..],
        "service_account" | "vertex_ai" => &["claude:chat", "gemini:chat"][..],
        _ => return Ok(()),
    };
    let invalid = api_formats
        .iter()
        .filter(|value| !allowed.contains(&value.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Vertex {auth_type} 不支持以下 API 格式: {}；允许: {}",
        invalid.join(", "),
        allowed.join(", ")
    ))
}

mod keys;
mod provider;
mod reveal;

pub(crate) use self::keys::{
    build_admin_create_provider_key_record, build_admin_provider_keys_payload,
    build_admin_update_provider_key_record,
};
pub(crate) use self::provider::{
    build_admin_create_provider_record, build_admin_fixed_provider_endpoint_record,
    build_admin_update_provider_record,
};
pub(crate) use self::reveal::{build_admin_export_key_payload, build_admin_reveal_key_payload};
