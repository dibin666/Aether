use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredRequestUsageAudit {
    pub id: String,
    pub request_id: String,
    pub user_id: Option<String>,
    pub api_key_id: Option<String>,
    pub username: Option<String>,
    pub api_key_name: Option<String>,
    pub provider_name: String,
    pub model: String,
    pub target_model: Option<String>,
    pub provider_id: Option<String>,
    pub provider_endpoint_id: Option<String>,
    pub provider_api_key_id: Option<String>,
    pub request_type: Option<String>,
    pub api_format: Option<String>,
    pub api_family: Option<String>,
    pub endpoint_kind: Option<String>,
    pub endpoint_api_format: Option<String>,
    pub provider_api_family: Option<String>,
    pub provider_endpoint_kind: Option<String>,
    pub has_format_conversion: bool,
    pub is_stream: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub actual_total_cost_usd: f64,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub error_category: Option<String>,
    pub response_time_ms: Option<u64>,
    pub first_byte_time_ms: Option<u64>,
    pub status: String,
    pub billing_status: String,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
    pub finalized_at_unix_secs: Option<u64>,
}

impl StoredRequestUsageAudit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        request_id: String,
        user_id: Option<String>,
        api_key_id: Option<String>,
        username: Option<String>,
        api_key_name: Option<String>,
        provider_name: String,
        model: String,
        target_model: Option<String>,
        provider_id: Option<String>,
        provider_endpoint_id: Option<String>,
        provider_api_key_id: Option<String>,
        request_type: Option<String>,
        api_format: Option<String>,
        api_family: Option<String>,
        endpoint_kind: Option<String>,
        endpoint_api_format: Option<String>,
        provider_api_family: Option<String>,
        provider_endpoint_kind: Option<String>,
        has_format_conversion: bool,
        is_stream: bool,
        input_tokens: i32,
        output_tokens: i32,
        total_tokens: i32,
        total_cost_usd: f64,
        actual_total_cost_usd: f64,
        status_code: Option<i32>,
        error_message: Option<String>,
        error_category: Option<String>,
        response_time_ms: Option<i32>,
        first_byte_time_ms: Option<i32>,
        status: String,
        billing_status: String,
        created_at_unix_secs: i64,
        updated_at_unix_secs: i64,
        finalized_at_unix_secs: Option<i64>,
    ) -> Result<Self, crate::DataLayerError> {
        if request_id.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.request_id is empty".to_string(),
            ));
        }
        if provider_name.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.provider_name is empty".to_string(),
            ));
        }
        if model.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.model is empty".to_string(),
            ));
        }
        if status.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.status is empty".to_string(),
            ));
        }
        if billing_status.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.billing_status is empty".to_string(),
            ));
        }
        if !total_cost_usd.is_finite() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.total_cost_usd is not finite".to_string(),
            ));
        }
        if !actual_total_cost_usd.is_finite() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "usage.actual_total_cost_usd is not finite".to_string(),
            ));
        }

        Ok(Self {
            id,
            request_id,
            user_id,
            api_key_id,
            username,
            api_key_name,
            provider_name,
            model,
            target_model,
            provider_id,
            provider_endpoint_id,
            provider_api_key_id,
            request_type,
            api_format,
            api_family,
            endpoint_kind,
            endpoint_api_format,
            provider_api_family,
            provider_endpoint_kind,
            has_format_conversion,
            is_stream,
            input_tokens: parse_u64(input_tokens, "usage.input_tokens")?,
            output_tokens: parse_u64(output_tokens, "usage.output_tokens")?,
            total_tokens: parse_u64(total_tokens, "usage.total_tokens")?,
            total_cost_usd,
            actual_total_cost_usd,
            status_code: parse_u16(status_code, "usage.status_code")?,
            error_message,
            error_category,
            response_time_ms: parse_optional_u64(response_time_ms, "usage.response_time_ms")?,
            first_byte_time_ms: parse_optional_u64(first_byte_time_ms, "usage.first_byte_time_ms")?,
            status,
            billing_status,
            created_at_unix_secs: parse_timestamp(
                created_at_unix_secs,
                "usage.created_at_unix_secs",
            )?,
            updated_at_unix_secs: parse_timestamp(
                updated_at_unix_secs,
                "usage.updated_at_unix_secs",
            )?,
            finalized_at_unix_secs: finalized_at_unix_secs
                .map(|value| parse_timestamp(value, "usage.finalized_at_unix_secs"))
                .transpose()?,
        })
    }
}

#[async_trait]
pub trait UsageReadRepository: Send + Sync {
    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<StoredRequestUsageAudit>, crate::DataLayerError>;
}

pub trait UsageRepository: UsageReadRepository + Send + Sync {}

impl<T> UsageRepository for T where T: UsageReadRepository + Send + Sync {}

fn parse_u64(value: i32, field_name: &str) -> Result<u64, crate::DataLayerError> {
    u64::try_from(value).map_err(|_| {
        crate::DataLayerError::UnexpectedValue(format!("invalid {field_name}: {value}"))
    })
}

fn parse_optional_u64(
    value: Option<i32>,
    field_name: &str,
) -> Result<Option<u64>, crate::DataLayerError> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                crate::DataLayerError::UnexpectedValue(format!("invalid {field_name}: {value}"))
            })
        })
        .transpose()
}

fn parse_u16(value: Option<i32>, field_name: &str) -> Result<Option<u16>, crate::DataLayerError> {
    value
        .map(|value| {
            u16::try_from(value).map_err(|_| {
                crate::DataLayerError::UnexpectedValue(format!("invalid {field_name}: {value}"))
            })
        })
        .transpose()
}

fn parse_timestamp(value: i64, field_name: &str) -> Result<u64, crate::DataLayerError> {
    u64::try_from(value).map_err(|_| {
        crate::DataLayerError::UnexpectedValue(format!("invalid {field_name}: {value}"))
    })
}

#[cfg(test)]
mod tests {
    use super::StoredRequestUsageAudit;

    #[test]
    fn rejects_empty_request_id() {
        assert!(StoredRequestUsageAudit::new(
            "usage-1".to_string(),
            "".to_string(),
            None,
            None,
            None,
            None,
            "OpenAI".to_string(),
            "gpt-4.1".to_string(),
            None,
            None,
            None,
            None,
            Some("chat".to_string()),
            Some("openai:chat".to_string()),
            Some("openai".to_string()),
            Some("chat".to_string()),
            Some("openai:chat".to_string()),
            Some("openai".to_string()),
            Some("chat".to_string()),
            false,
            false,
            10,
            20,
            30,
            0.1,
            0.1,
            Some(200),
            None,
            None,
            Some(120),
            Some(80),
            "completed".to_string(),
            "settled".to_string(),
            100,
            101,
            Some(102),
        )
        .is_err());
    }

    #[test]
    fn rejects_negative_token_count() {
        assert!(StoredRequestUsageAudit::new(
            "usage-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            "OpenAI".to_string(),
            "gpt-4.1".to_string(),
            None,
            None,
            None,
            None,
            Some("chat".to_string()),
            Some("openai:chat".to_string()),
            Some("openai".to_string()),
            Some("chat".to_string()),
            Some("openai:chat".to_string()),
            Some("openai".to_string()),
            Some("chat".to_string()),
            false,
            false,
            -1,
            20,
            30,
            0.1,
            0.1,
            Some(200),
            None,
            None,
            Some(120),
            Some(80),
            "completed".to_string(),
            "settled".to_string(),
            100,
            101,
            Some(102),
        )
        .is_err());
    }
}
