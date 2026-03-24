use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestCandidateStatus {
    Available,
    Unused,
    Pending,
    Streaming,
    Success,
    Failed,
    Cancelled,
    Skipped,
}

impl RequestCandidateStatus {
    pub fn from_database(value: &str) -> Result<Self, crate::DataLayerError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "available" => Ok(Self::Available),
            "unused" => Ok(Self::Unused),
            "pending" => Ok(Self::Pending),
            "streaming" => Ok(Self::Streaming),
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            other => Err(crate::DataLayerError::UnexpectedValue(format!(
                "unsupported request_candidates.status: {other}"
            ))),
        }
    }

    pub fn is_attempted(self, started_at_unix_secs: Option<u64>) -> bool {
        match self {
            Self::Available | Self::Unused | Self::Skipped => false,
            Self::Pending => started_at_unix_secs.is_some(),
            Self::Streaming | Self::Success | Self::Failed | Self::Cancelled => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredRequestCandidate {
    pub id: String,
    pub request_id: String,
    pub user_id: Option<String>,
    pub api_key_id: Option<String>,
    pub username: Option<String>,
    pub api_key_name: Option<String>,
    pub candidate_index: u32,
    pub retry_index: u32,
    pub provider_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub key_id: Option<String>,
    pub status: RequestCandidateStatus,
    pub skip_reason: Option<String>,
    pub is_cached: bool,
    pub status_code: Option<u16>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub latency_ms: Option<u64>,
    pub concurrent_requests: Option<u32>,
    pub extra_data: Option<serde_json::Value>,
    pub required_capabilities: Option<serde_json::Value>,
    pub created_at_unix_secs: u64,
    pub started_at_unix_secs: Option<u64>,
    pub finished_at_unix_secs: Option<u64>,
}

impl StoredRequestCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        request_id: String,
        user_id: Option<String>,
        api_key_id: Option<String>,
        username: Option<String>,
        api_key_name: Option<String>,
        candidate_index: i32,
        retry_index: i32,
        provider_id: Option<String>,
        endpoint_id: Option<String>,
        key_id: Option<String>,
        status: RequestCandidateStatus,
        skip_reason: Option<String>,
        is_cached: bool,
        status_code: Option<i32>,
        error_type: Option<String>,
        error_message: Option<String>,
        latency_ms: Option<i32>,
        concurrent_requests: Option<i32>,
        extra_data: Option<serde_json::Value>,
        required_capabilities: Option<serde_json::Value>,
        created_at_unix_secs: i64,
        started_at_unix_secs: Option<i64>,
        finished_at_unix_secs: Option<i64>,
    ) -> Result<Self, crate::DataLayerError> {
        let candidate_index = u32::try_from(candidate_index).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid request_candidates.candidate_index: {candidate_index}"
            ))
        })?;
        let retry_index = u32::try_from(retry_index).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid request_candidates.retry_index: {retry_index}"
            ))
        })?;
        let status_code = status_code
            .map(|value| {
                u16::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!(
                        "invalid request_candidates.status_code: {value}"
                    ))
                })
            })
            .transpose()?;
        let latency_ms = latency_ms
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!(
                        "invalid request_candidates.latency_ms: {value}"
                    ))
                })
            })
            .transpose()?;
        let concurrent_requests = concurrent_requests
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!(
                        "invalid request_candidates.concurrent_requests: {value}"
                    ))
                })
            })
            .transpose()?;
        let created_at_unix_secs = u64::try_from(created_at_unix_secs).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid request_candidates.created_at_unix_secs: {created_at_unix_secs}"
            ))
        })?;
        let started_at_unix_secs = started_at_unix_secs
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!(
                        "invalid request_candidates.started_at_unix_secs: {value}"
                    ))
                })
            })
            .transpose()?;
        let finished_at_unix_secs = finished_at_unix_secs
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!(
                        "invalid request_candidates.finished_at_unix_secs: {value}"
                    ))
                })
            })
            .transpose()?;

        Ok(Self {
            id,
            request_id,
            user_id,
            api_key_id,
            username,
            api_key_name,
            candidate_index,
            retry_index,
            provider_id,
            endpoint_id,
            key_id,
            status,
            skip_reason,
            is_cached,
            status_code,
            error_type,
            error_message,
            latency_ms,
            concurrent_requests,
            extra_data,
            required_capabilities,
            created_at_unix_secs,
            started_at_unix_secs,
            finished_at_unix_secs,
        })
    }
}

#[async_trait]
pub trait RequestCandidateReadRepository: Send + Sync {
    async fn list_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, crate::DataLayerError>;

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredRequestCandidate>, crate::DataLayerError>;
}

pub trait RequestCandidateRepository: RequestCandidateReadRepository + Send + Sync {}

impl<T> RequestCandidateRepository for T where T: RequestCandidateReadRepository + Send + Sync {}

#[cfg(test)]
mod tests {
    use super::{RequestCandidateStatus, StoredRequestCandidate};

    #[test]
    fn parses_status_from_database_text() {
        assert_eq!(
            RequestCandidateStatus::from_database("streaming").expect("status should parse"),
            RequestCandidateStatus::Streaming
        );
    }

    #[test]
    fn rejects_invalid_database_status() {
        assert!(RequestCandidateStatus::from_database("mystery").is_err());
    }

    #[test]
    fn rejects_negative_candidate_index() {
        assert!(StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            -1,
            0,
            None,
            None,
            None,
            RequestCandidateStatus::Pending,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            Some(1),
            None,
            None,
            100,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn rejects_negative_created_at() {
        assert!(StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            None,
            None,
            None,
            RequestCandidateStatus::Pending,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            Some(1),
            None,
            None,
            -1,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn pending_without_started_at_is_not_attempted() {
        assert!(!RequestCandidateStatus::Pending.is_attempted(None));
        assert!(RequestCandidateStatus::Pending.is_attempted(Some(1)));
    }
}
