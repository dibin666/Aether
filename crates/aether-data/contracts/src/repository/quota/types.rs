use async_trait::async_trait;
use serde_json::Value;

pub const PROVIDER_KEY_QUOTA_BUCKET_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderKeyQuotaWindowObservation {
    pub window_identity: String,
    pub code: String,
    pub label: String,
    pub scope: Option<String>,
    pub model: Option<String>,
    pub unit: Option<String>,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub used_value: Option<f64>,
    pub remaining_value: Option<f64>,
    pub limit_value: Option<f64>,
    pub reset_at_unix_secs: Option<u64>,
    pub window_minutes: Option<u64>,
    pub exhausted: bool,
    pub local_request_count: u64,
    pub local_total_tokens: u64,
    pub local_cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderKeyQuotaObservation {
    pub provider_id: String,
    pub provider_api_key_id: String,
    pub provider_api_key_name: String,
    pub provider_type: String,
    pub bucket_start_unix_secs: u64,
    pub observed_at_unix_secs: u64,
    pub source: String,
    pub plan_type: Option<String>,
    pub status_code: Option<String>,
    pub status_label: Option<String>,
    pub freshness: Option<String>,
    pub credits_balance: Option<f64>,
    pub credits_unlimited: Option<bool>,
    pub reset_credits_count: u64,
    pub windows: Vec<ProviderKeyQuotaWindowObservation>,
}

impl ProviderKeyQuotaObservation {
    pub fn from_status_snapshot(
        provider_id: impl Into<String>,
        provider_api_key_id: impl Into<String>,
        provider_api_key_name: impl Into<String>,
        provider_type: impl Into<String>,
        status_snapshot: &Value,
        fallback_observed_at_unix_secs: u64,
    ) -> Option<Self> {
        let quota = status_snapshot.get("quota")?.as_object()?;
        let observed_at_unix_secs = json_u64(quota.get("observed_at"))
            .or_else(|| json_u64(quota.get("updated_at")))
            .unwrap_or(fallback_observed_at_unix_secs);
        let bucket_start_unix_secs = observed_at_unix_secs
            .saturating_sub(observed_at_unix_secs % PROVIDER_KEY_QUOTA_BUCKET_SECS);
        let source = json_string(quota.get("source")).unwrap_or_else(|| "status_snapshot".into());
        let credits = quota.get("credits").and_then(Value::as_object);
        let credits_balance = credits
            .and_then(|value| {
                json_f64(value.get("balance"))
                    .or_else(|| json_f64(value.get("remaining")))
                    .or_else(|| json_f64(value.get("available")))
            })
            .or_else(|| json_f64(quota.get("credits_balance")));
        let credits_unlimited = credits
            .and_then(|value| {
                value
                    .get("unlimited")
                    .and_then(Value::as_bool)
                    .or_else(|| value.get("is_unlimited").and_then(Value::as_bool))
            })
            .or_else(|| quota.get("credits_unlimited").and_then(Value::as_bool));
        let reset_credits_count = quota
            .get("reset_credits")
            .and_then(|value| {
                value
                    .as_array()
                    .map(|items| items.len() as u64)
                    .or_else(|| {
                        value
                            .get("available_count")
                            .and_then(|count| json_u64(Some(count)))
                    })
                    .or_else(|| {
                        value
                            .get("credits")
                            .and_then(Value::as_array)
                            .map(|items| items.len() as u64)
                    })
                    .or_else(|| json_u64(Some(value)))
            })
            .or_else(|| json_u64(quota.get("reset_credits_count")))
            .unwrap_or(0);

        let windows = quota
            .get("windows")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, value)| {
                        parse_window_observation(value, index, observed_at_unix_secs)
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            provider_id: provider_id.into(),
            provider_api_key_id: provider_api_key_id.into(),
            provider_api_key_name: provider_api_key_name.into(),
            provider_type: provider_type.into(),
            bucket_start_unix_secs,
            observed_at_unix_secs,
            source,
            plan_type: json_string(quota.get("plan_type"))
                .or_else(|| json_string(quota.get("plan"))),
            status_code: json_string(quota.get("code")),
            status_label: json_string(quota.get("label")),
            freshness: json_string(quota.get("freshness")),
            credits_balance,
            credits_unlimited,
            reset_credits_count,
            windows,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ProviderKeyQuotaObservationQuery {
    pub provider_id: String,
    pub provider_api_key_id: Option<String>,
    pub observed_from_unix_secs: Option<u64>,
    pub observed_until_unix_secs: Option<u64>,
    pub limit: Option<usize>,
}

fn parse_window_observation(
    value: &Value,
    index: usize,
    observed_at_unix_secs: u64,
) -> Option<ProviderKeyQuotaWindowObservation> {
    let window = value.as_object()?;
    let code = json_string(window.get("code")).unwrap_or_else(|| format!("window_{index}"));
    let label = json_string(window.get("label")).unwrap_or_else(|| code.clone());
    let scope = json_string(window.get("scope"));
    let model = json_string(window.get("model"))
        .or_else(|| json_string(window.get("model_id")))
        .or_else(|| json_string(window.get("model_name")));
    let unit = json_string(window.get("unit"));
    let used_percent = json_f64(window.get("used_percent"))
        .or_else(|| json_f64(window.get("usage_percentage")))
        .or_else(|| json_f64(window.get("percentage_used")))
        .or_else(|| json_f64(window.get("used_ratio")).map(ratio_to_percent))
        .map(clamp_percent);
    let remaining_percent = json_f64(window.get("remaining_percent"))
        .map(clamp_percent)
        .or_else(|| json_f64(window.get("remaining_ratio")).map(ratio_to_percent))
        .or_else(|| used_percent.map(|value| 100.0 - value));
    let used_value = json_f64(window.get("used")).or_else(|| json_f64(window.get("used_value")));
    let remaining_value =
        json_f64(window.get("remaining")).or_else(|| json_f64(window.get("remaining_value")));
    let limit_value = json_f64(window.get("limit"))
        .or_else(|| json_f64(window.get("limit_value")))
        .or_else(|| json_f64(window.get("total")));
    let reset_at_unix_secs = json_u64(window.get("reset_at"))
        .or_else(|| json_u64(window.get("reset_at_unix_secs")))
        .or_else(|| {
            json_u64(window.get("reset_after_seconds"))
                .map(|seconds| observed_at_unix_secs.saturating_add(seconds))
        })
        .or_else(|| {
            json_u64(window.get("reset_seconds"))
                .map(|seconds| observed_at_unix_secs.saturating_add(seconds))
        });
    let window_minutes = json_u64(window.get("window_minutes"))
        .or_else(|| json_u64(window.get("limit_window_minutes")))
        .or_else(|| json_u64(window.get("window_seconds")).map(|value| value / 60));
    let exhausted = window
        .get("exhausted")
        .and_then(Value::as_bool)
        .or_else(|| window.get("is_exhausted").and_then(Value::as_bool))
        .unwrap_or_else(|| remaining_percent.is_some_and(|value| value <= 0.0));
    let usage = window.get("usage").and_then(Value::as_object);
    let local_request_count = usage
        .and_then(|item| json_u64(item.get("request_count")))
        .unwrap_or(0);
    let local_total_tokens = usage
        .and_then(|item| json_u64(item.get("total_tokens")))
        .unwrap_or(0);
    let local_cost_usd = usage
        .and_then(|item| json_f64(item.get("total_cost_usd")))
        .unwrap_or(0.0);
    let window_identity = format!(
        "{}|{}|{}|{}",
        code,
        scope.as_deref().unwrap_or_default(),
        model.as_deref().unwrap_or_default(),
        index
    );

    Some(ProviderKeyQuotaWindowObservation {
        window_identity,
        code,
        label,
        scope,
        model,
        unit,
        used_percent,
        remaining_percent,
        used_value,
        remaining_value,
        limit_value,
        reset_at_unix_secs,
        window_minutes,
        exhausted,
        local_request_count,
        local_total_tokens,
        local_cost_usd,
    })
}

fn json_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn json_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
            .or_else(|| {
                value
                    .as_f64()
                    .filter(|value| *value >= 0.0)
                    .map(|value| value as u64)
            })
            .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
    })
}

fn json_f64(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        })
        .filter(|value| value.is_finite())
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn ratio_to_percent(value: f64) -> f64 {
    if value.abs() <= 1.0 {
        value * 100.0
    } else {
        value
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredProviderQuotaSnapshot {
    pub provider_id: String,
    pub billing_type: String,
    pub monthly_quota_usd: Option<f64>,
    pub monthly_used_usd: f64,
    pub quota_reset_day: Option<u64>,
    pub quota_last_reset_at_unix_secs: Option<u64>,
    pub quota_expires_at_unix_secs: Option<u64>,
    pub is_active: bool,
}

impl StoredProviderQuotaSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: String,
        billing_type: String,
        monthly_quota_usd: Option<f64>,
        monthly_used_usd: f64,
        quota_reset_day: Option<i32>,
        quota_last_reset_at_unix_secs: Option<i64>,
        quota_expires_at_unix_secs: Option<i64>,
        is_active: bool,
    ) -> Result<Self, crate::DataLayerError> {
        if provider_id.trim().is_empty() || billing_type.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "provider quota identity is empty".to_string(),
            ));
        }
        if !monthly_used_usd.is_finite() || monthly_quota_usd.is_some_and(|v| !v.is_finite()) {
            return Err(crate::DataLayerError::UnexpectedValue(
                "provider quota value is not finite".to_string(),
            ));
        }
        Ok(Self {
            provider_id,
            billing_type,
            monthly_quota_usd,
            monthly_used_usd,
            quota_reset_day: quota_reset_day.map(|value| value as u64),
            quota_last_reset_at_unix_secs: quota_last_reset_at_unix_secs.map(|value| value as u64),
            quota_expires_at_unix_secs: quota_expires_at_unix_secs.map(|value| value as u64),
            is_active,
        })
    }
}

#[async_trait]
pub trait ProviderQuotaReadRepository: Send + Sync {
    async fn find_by_provider_id(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, crate::DataLayerError>;

    async fn find_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderQuotaSnapshot>, crate::DataLayerError>;

    async fn list_key_quota_observations(
        &self,
        query: &ProviderKeyQuotaObservationQuery,
    ) -> Result<Vec<ProviderKeyQuotaObservation>, crate::DataLayerError>;
}

#[async_trait]
pub trait ProviderQuotaWriteRepository: Send + Sync {
    async fn reset_due(&self, now_unix_secs: u64) -> Result<usize, crate::DataLayerError>;

    async fn upsert_key_quota_observation(
        &self,
        observation: &ProviderKeyQuotaObservation,
    ) -> Result<bool, crate::DataLayerError>;
}

pub trait ProviderQuotaRepository:
    ProviderQuotaReadRepository + ProviderQuotaWriteRepository + Send + Sync
{
}

impl<T> ProviderQuotaRepository for T where
    T: ProviderQuotaReadRepository + ProviderQuotaWriteRepository + Send + Sync
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_generic_quota_snapshot_into_five_minute_bucket() {
        let observation = ProviderKeyQuotaObservation::from_status_snapshot(
            "provider-1",
            "key-1",
            "Key One",
            "codex",
            &json!({
                "quota": {
                    "observed_at": 1_721,
                    "source": "response_headers",
                    "plan_type": "plus",
                    "credits": { "balance": "12.5", "unlimited": false },
                    "reset_credits": [{"id": 1}, {"id": 2}],
                    "windows": [{
                        "code": "weekly",
                        "label": "周额度",
                        "used_percent": 42.5,
                        "reset_after_seconds": 600,
                        "window_minutes": 10_080
                    }]
                }
            }),
            1_700,
        )
        .expect("quota should normalize");

        assert_eq!(observation.bucket_start_unix_secs, 1_500);
        assert_eq!(observation.observed_at_unix_secs, 1_721);
        assert_eq!(observation.source, "response_headers");
        assert_eq!(observation.credits_balance, Some(12.5));
        assert_eq!(observation.reset_credits_count, 2);
        assert_eq!(observation.windows[0].remaining_percent, Some(57.5));
        assert_eq!(observation.windows[0].reset_at_unix_secs, Some(2_321));
    }

    #[test]
    fn count_windows_without_percent_do_not_invent_percentages() {
        let observation = ProviderKeyQuotaObservation::from_status_snapshot(
            "provider-1",
            "key-1",
            "Key One",
            "kiro",
            &json!({
                "quota": { "windows": [{ "code": "credits", "remaining": 4, "unit": "credits" }] }
            }),
            900,
        )
        .expect("quota should normalize");

        assert_eq!(observation.windows[0].remaining_value, Some(4.0));
        assert_eq!(observation.windows[0].remaining_percent, None);
    }

    #[test]
    fn normalizes_aether_ratio_window_fields_and_reset_credit_count() {
        let observation = ProviderKeyQuotaObservation::from_status_snapshot(
            "provider-1",
            "key-1",
            "Key One",
            "windsurf",
            &json!({
                "quota": {
                    "updated_at": 10_000,
                    "reset_credits": { "available_count": 3 },
                    "windows": [{
                        "code": "weekly",
                        "scope": "account",
                        "unit": "percent",
                        "used_ratio": 0.25,
                        "remaining_ratio": 0.75,
                        "reset_seconds": 60,
                        "is_exhausted": false
                    }]
                }
            }),
            1,
        )
        .expect("quota should normalize");

        assert_eq!(observation.reset_credits_count, 3);
        assert_eq!(observation.windows[0].used_percent, Some(25.0));
        assert_eq!(observation.windows[0].remaining_percent, Some(75.0));
        assert_eq!(observation.windows[0].reset_at_unix_secs, Some(10_060));
        assert!(!observation.windows[0].exhausted);
    }
}
