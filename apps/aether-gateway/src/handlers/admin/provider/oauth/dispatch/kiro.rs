use crate::handlers::admin::provider::oauth::state::{current_unix_secs, json_non_empty_string};
use crate::handlers::admin::provider::shared::payloads::{
    KIRO_USAGE_LIMITS_PATH, KIRO_USAGE_SDK_VERSION,
};
use crate::handlers::admin::request::{AdminAppState, AdminKiroAuthConfig};
use crate::provider_transport::kiro::{build_kiro_request_auth_from_config, KiroRequestAuth};
use aether_contracts::ProxySnapshot;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

const KIRO_IDC_AMZ_USER_AGENT: &str =
    "aws-sdk-js/3.738.0 ua/2.1 os/other lang/js md/browser#unknown_unknown api/sso-oidc#3.738.0 m/E KiroIDE";

pub(super) fn admin_provider_oauth_kiro_refresh_base_url_override(
    state: &AdminAppState<'_>,
    override_key: &str,
) -> Option<String> {
    let override_url = state.app().provider_oauth_token_url(override_key, "");
    let normalized = override_url.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn admin_provider_oauth_kiro_build_refresh_url(
    auth_config: &AdminKiroAuthConfig,
    override_base_url: Option<&str>,
    path: &str,
    default_host: impl FnOnce(&str) -> String,
) -> String {
    if let Some(base_url) = override_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}/{}", base_url.trim_end_matches('/'), path);
    }
    let region = auth_config.effective_auth_region();
    default_host(region)
}

fn admin_provider_oauth_kiro_effective_host(url: &str, fallback_host: String) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|value| value.host_str().map(ToOwned::to_owned))
        .unwrap_or(fallback_host)
}

fn admin_provider_oauth_kiro_ide_tag(kiro_version: &str, machine_id: &str) -> String {
    if machine_id.trim().is_empty() {
        format!("KiroIDE-{kiro_version}")
    } else {
        format!("KiroIDE-{kiro_version}-{machine_id}")
    }
}

fn admin_provider_oauth_kiro_refresh_expires_at(payload: &Value) -> u64 {
    let expires_in = payload
        .get("expiresIn")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok())
        })
        .unwrap_or(3600);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_secs())
        .unwrap_or_default()
        .saturating_add(expires_in)
}

fn admin_provider_oauth_kiro_refresh_response_json(
    body_text: &str,
    json_body: Option<Value>,
) -> Result<Value, String> {
    json_body
        .or_else(|| serde_json::from_str::<Value>(body_text).ok())
        .ok_or_else(|| "refresh 接口返回了非 JSON 响应".to_string())
}

fn admin_provider_oauth_kiro_refresh_error_detail(
    status: http::StatusCode,
    body_text: &str,
) -> String {
    let detail = body_text.trim();
    if detail.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        detail.to_string()
    }
}

pub(super) async fn refresh_admin_provider_oauth_kiro_auth_config(
    state: &AdminAppState<'_>,
    auth_config: &AdminKiroAuthConfig,
    proxy: Option<ProxySnapshot>,
    social_refresh_base_url: Option<&str>,
    idc_refresh_base_url: Option<&str>,
) -> Result<AdminKiroAuthConfig, String> {
    if auth_config.is_idc_auth() {
        let fallback_host = format!("oidc.{}.amazonaws.com", auth_config.effective_auth_region());
        let url = admin_provider_oauth_kiro_build_refresh_url(
            auth_config,
            idc_refresh_base_url,
            "token",
            |region| format!("https://oidc.{region}.amazonaws.com/token"),
        );
        let host = admin_provider_oauth_kiro_effective_host(&url, fallback_host);
        let headers = reqwest::header::HeaderMap::from_iter([
            (
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            ),
            (
                reqwest::header::HOST,
                reqwest::header::HeaderValue::from_str(&host)
                    .map_err(|_| "IDC host 无效".to_string())?,
            ),
            (
                reqwest::header::HeaderName::from_static("x-amz-user-agent"),
                reqwest::header::HeaderValue::from_static(KIRO_IDC_AMZ_USER_AGENT),
            ),
            (
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_static("node"),
            ),
            (
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("*/*"),
            ),
        ]);
        let response = state
            .execute_admin_provider_oauth_http_request(
                "kiro_batch_refresh:idc",
                reqwest::Method::POST,
                &url,
                &headers,
                Some("application/json"),
                Some(json!({
                    "clientId": auth_config
                        .client_id
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default(),
                    "clientSecret": auth_config
                        .client_secret
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default(),
                    "refreshToken": auth_config
                        .refresh_token
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default(),
                    "grantType": "refresh_token",
                })),
                None,
                proxy.clone(),
            )
            .await
            .map_err(|err| format!("IDC refresh 请求失败: {err}"))?;
        if !response.status.is_success() {
            return Err(format!(
                "IDC refresh 失败: {}",
                admin_provider_oauth_kiro_refresh_error_detail(
                    response.status,
                    &response.body_text
                )
            ));
        }
        let payload = admin_provider_oauth_kiro_refresh_response_json(
            &response.body_text,
            response.json_body,
        )?;
        let access_token = payload
            .get("accessToken")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "IDC refresh 返回了空 accessToken".to_string())?;

        let mut refreshed = auth_config.clone();
        refreshed.access_token = Some(access_token.to_string());
        refreshed.expires_at = Some(admin_provider_oauth_kiro_refresh_expires_at(&payload));
        if refreshed
            .machine_id
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
        {
            refreshed.machine_id =
                crate::provider_transport::kiro::generate_machine_id(auth_config, None);
        }
        if let Some(refresh_token) = payload
            .get("refreshToken")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            refreshed.refresh_token = Some(refresh_token.to_string());
        }
        return Ok(refreshed);
    }

    let machine_id = crate::provider_transport::kiro::generate_machine_id(auth_config, None)
        .ok_or_else(|| "缺少 machine_id 种子，无法刷新 social token".to_string())?;
    let fallback_host = format!(
        "prod.{}.auth.desktop.kiro.dev",
        auth_config.effective_auth_region()
    );
    let url = admin_provider_oauth_kiro_build_refresh_url(
        auth_config,
        social_refresh_base_url,
        "refreshToken",
        |region| format!("https://prod.{region}.auth.desktop.kiro.dev/refreshToken"),
    );
    let host = admin_provider_oauth_kiro_effective_host(&url, fallback_host);
    let user_agent =
        admin_provider_oauth_kiro_ide_tag(auth_config.effective_kiro_version(), &machine_id);
    let headers = reqwest::header::HeaderMap::from_iter([
        (
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(&user_agent)
                .map_err(|_| "Kiro User-Agent 无效".to_string())?,
        ),
        (
            reqwest::header::HOST,
            reqwest::header::HeaderValue::from_str(&host)
                .map_err(|_| "Kiro host 无效".to_string())?,
        ),
        (
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"),
        ),
        (
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        ),
        (
            reqwest::header::CONNECTION,
            reqwest::header::HeaderValue::from_static("close"),
        ),
        (
            reqwest::header::ACCEPT_ENCODING,
            reqwest::header::HeaderValue::from_static("gzip, compress, deflate, br"),
        ),
    ]);
    let response = state
        .execute_admin_provider_oauth_http_request(
            "kiro_batch_refresh:social",
            reqwest::Method::POST,
            &url,
            &headers,
            Some("application/json"),
            Some(json!({
                "refreshToken": auth_config
                    .refresh_token
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default(),
            })),
            None,
            proxy,
        )
        .await
        .map_err(|err| format!("social refresh 请求失败: {err}"))?;
    if !response.status.is_success() {
        return Err(format!(
            "social refresh 失败: {}",
            admin_provider_oauth_kiro_refresh_error_detail(response.status, &response.body_text)
        ));
    }
    let payload =
        admin_provider_oauth_kiro_refresh_response_json(&response.body_text, response.json_body)?;
    let access_token = payload
        .get("accessToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "social refresh 返回了空 accessToken".to_string())?;

    let mut refreshed = auth_config.clone();
    refreshed.access_token = Some(access_token.to_string());
    refreshed.expires_at = Some(admin_provider_oauth_kiro_refresh_expires_at(&payload));
    if refreshed
        .machine_id
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        refreshed.machine_id = Some(machine_id);
    }
    if let Some(refresh_token) = payload
        .get("refreshToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        refreshed.refresh_token = Some(refresh_token.to_string());
    }
    if let Some(profile_arn) = payload
        .get("profileArn")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        refreshed.profile_arn = Some(profile_arn.to_string());
    }
    Ok(refreshed)
}

fn build_kiro_usage_url(auth: &KiroRequestAuth) -> String {
    let host = format!(
        "q.{}.amazonaws.com",
        auth.auth_config.effective_api_region()
    );
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("origin", "AI_EDITOR");
    serializer.append_pair("resourceType", "AGENTIC_REQUEST");
    serializer.append_pair("isEmailRequired", "true");
    if let Some(profile_arn) = auth.auth_config.profile_arn_for_payload() {
        serializer.append_pair("profileArn", profile_arn);
    }
    format!(
        "https://{host}{KIRO_USAGE_LIMITS_PATH}?{}",
        serializer.finish()
    )
}

fn build_kiro_usage_headers(
    auth: &KiroRequestAuth,
    host: &str,
) -> Result<reqwest::header::HeaderMap, String> {
    let kiro_version = auth.auth_config.effective_kiro_version();
    let machine_id = auth.machine_id.trim();
    let ide_tag = admin_provider_oauth_kiro_ide_tag(kiro_version, machine_id);
    Ok(reqwest::header::HeaderMap::from_iter([
        (
            reqwest::header::HeaderName::from_static("x-amz-user-agent"),
            reqwest::header::HeaderValue::from_str(&format!(
                "aws-sdk-js/{KIRO_USAGE_SDK_VERSION} {ide_tag}"
            ))
            .map_err(|_| "Kiro usage x-amz-user-agent 无效".to_string())?,
        ),
        (
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(&format!(
                "aws-sdk-js/{KIRO_USAGE_SDK_VERSION} ua/2.1 os/other#unknown lang/js md/nodejs#22.21.1 api/codewhispererruntime#1.0.0 m/N,E {ide_tag}"
            ))
            .map_err(|_| "Kiro usage User-Agent 无效".to_string())?,
        ),
        (
            reqwest::header::HOST,
            reqwest::header::HeaderValue::from_str(host)
                .map_err(|_| "Kiro usage host 无效".to_string())?,
        ),
        (
            reqwest::header::HeaderName::from_static("amz-sdk-invocation-id"),
            reqwest::header::HeaderValue::from_str(&uuid::Uuid::new_v4().to_string())
                .map_err(|_| "Kiro usage invocation id 无效".to_string())?,
        ),
        (
            reqwest::header::HeaderName::from_static("amz-sdk-request"),
            reqwest::header::HeaderValue::from_static("attempt=1; max=1"),
        ),
        (
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(auth.value.as_str())
                .map_err(|_| "Kiro usage authorization 无效".to_string())?,
        ),
        (
            reqwest::header::CONNECTION,
            reqwest::header::HeaderValue::from_static("close"),
        ),
    ]))
}

pub(super) async fn fetch_admin_provider_oauth_kiro_email(
    state: &AdminAppState<'_>,
    auth_config: &AdminKiroAuthConfig,
    proxy: Option<ProxySnapshot>,
) -> Option<String> {
    let request_auth = build_kiro_request_auth_from_config(auth_config.clone(), None)?;
    let default_url = build_kiro_usage_url(&request_auth);
    let url = state
        .app()
        .provider_oauth_token_url("kiro_device_email", &default_url);
    let host = reqwest::Url::parse(&url)
        .ok()
        .and_then(|value| value.host_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| {
            format!(
                "q.{}.amazonaws.com",
                request_auth.auth_config.effective_api_region()
            )
        });
    let headers = build_kiro_usage_headers(&request_auth, &host).ok()?;
    let response = state
        .execute_admin_provider_oauth_http_request(
            "kiro_device_email",
            reqwest::Method::GET,
            &url,
            &headers,
            None,
            None,
            None,
            proxy,
        )
        .await
        .ok()?;
    if !response.status.is_success() {
        return None;
    }
    let payload = response
        .json_body
        .or_else(|| serde_json::from_str::<Value>(&response.body_text).ok())?;
    let metadata =
        aether_admin::provider::quota::parse_kiro_usage_response(&payload, current_unix_secs())?;
    json_non_empty_string(metadata.get("email"))
}
