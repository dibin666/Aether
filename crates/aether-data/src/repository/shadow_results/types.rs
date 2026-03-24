use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShadowResultMatchStatus {
    Pending,
    Match,
    Mismatch,
    Error,
}

impl ShadowResultMatchStatus {
    pub fn from_database(value: &str) -> Result<Self, crate::DataLayerError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "match" => Ok(Self::Match),
            "mismatch" => Ok(Self::Mismatch),
            "error" => Ok(Self::Error),
            other => Err(crate::DataLayerError::UnexpectedValue(format!(
                "unsupported gateway_shadow_results.match_status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredShadowResult {
    pub trace_id: String,
    pub request_fingerprint: String,
    pub request_id: Option<String>,
    pub route_family: Option<String>,
    pub route_kind: Option<String>,
    pub candidate_id: Option<String>,
    pub rust_result_digest: Option<String>,
    pub python_result_digest: Option<String>,
    pub match_status: ShadowResultMatchStatus,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
}

impl StoredShadowResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trace_id: String,
        request_fingerprint: String,
        request_id: Option<String>,
        route_family: Option<String>,
        route_kind: Option<String>,
        candidate_id: Option<String>,
        rust_result_digest: Option<String>,
        python_result_digest: Option<String>,
        match_status: ShadowResultMatchStatus,
        status_code: Option<i32>,
        error_message: Option<String>,
        created_at_unix_secs: i64,
        updated_at_unix_secs: i64,
    ) -> Result<Self, crate::DataLayerError> {
        let status_code = status_code
            .map(|value| {
                u16::try_from(value).map_err(|_| {
                    crate::DataLayerError::UnexpectedValue(format!("invalid status_code: {value}"))
                })
            })
            .transpose()?;
        let created_at_unix_secs = u64::try_from(created_at_unix_secs).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid created_at_unix_secs: {created_at_unix_secs}"
            ))
        })?;
        let updated_at_unix_secs = u64::try_from(updated_at_unix_secs).map_err(|_| {
            crate::DataLayerError::UnexpectedValue(format!(
                "invalid updated_at_unix_secs: {updated_at_unix_secs}"
            ))
        })?;

        Ok(Self {
            trace_id,
            request_fingerprint,
            request_id,
            route_family,
            route_kind,
            candidate_id,
            rust_result_digest,
            python_result_digest,
            match_status,
            status_code,
            error_message,
            created_at_unix_secs,
            updated_at_unix_secs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertShadowResult {
    pub trace_id: String,
    pub request_fingerprint: String,
    pub request_id: Option<String>,
    pub route_family: Option<String>,
    pub route_kind: Option<String>,
    pub candidate_id: Option<String>,
    pub rust_result_digest: Option<String>,
    pub python_result_digest: Option<String>,
    pub match_status: ShadowResultMatchStatus,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub created_at_unix_secs: u64,
    pub updated_at_unix_secs: u64,
}

impl UpsertShadowResult {
    pub fn into_stored(self) -> StoredShadowResult {
        StoredShadowResult {
            trace_id: self.trace_id,
            request_fingerprint: self.request_fingerprint,
            request_id: self.request_id,
            route_family: self.route_family,
            route_kind: self.route_kind,
            candidate_id: self.candidate_id,
            rust_result_digest: self.rust_result_digest,
            python_result_digest: self.python_result_digest,
            match_status: self.match_status,
            status_code: self.status_code,
            error_message: self.error_message,
            created_at_unix_secs: self.created_at_unix_secs,
            updated_at_unix_secs: self.updated_at_unix_secs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowResultLookupKey<'a> {
    TraceFingerprint {
        trace_id: &'a str,
        request_fingerprint: &'a str,
    },
}

#[async_trait]
pub trait ShadowResultReadRepository: Send + Sync {
    async fn find(
        &self,
        key: ShadowResultLookupKey<'_>,
    ) -> Result<Option<StoredShadowResult>, crate::DataLayerError>;

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredShadowResult>, crate::DataLayerError>;
}

#[async_trait]
pub trait ShadowResultWriteRepository: Send + Sync {
    async fn upsert(
        &self,
        result: UpsertShadowResult,
    ) -> Result<StoredShadowResult, crate::DataLayerError>;
}

pub trait ShadowResultRepository:
    ShadowResultReadRepository + ShadowResultWriteRepository + Send + Sync
{
}

impl<T> ShadowResultRepository for T where
    T: ShadowResultReadRepository + ShadowResultWriteRepository + Send + Sync
{
}

#[cfg(test)]
mod tests {
    use super::{ShadowResultMatchStatus, StoredShadowResult};

    #[test]
    fn parses_match_status_from_database_text() {
        assert_eq!(
            ShadowResultMatchStatus::from_database("match").expect("status should parse"),
            ShadowResultMatchStatus::Match
        );
    }

    #[test]
    fn rejects_invalid_database_status() {
        assert!(ShadowResultMatchStatus::from_database("mystery").is_err());
    }

    #[test]
    fn rejects_invalid_numeric_fields() {
        assert!(StoredShadowResult::new(
            "trace-1".to_string(),
            "fp-1".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            ShadowResultMatchStatus::Pending,
            Some(-1),
            None,
            1,
            1,
        )
        .is_err());
    }

    #[test]
    fn rejects_negative_updated_at_values() {
        assert!(StoredShadowResult::new(
            "trace-1".to_string(),
            "fp-1".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            ShadowResultMatchStatus::Pending,
            Some(200),
            None,
            1,
            -1,
        )
        .is_err());
    }
}
