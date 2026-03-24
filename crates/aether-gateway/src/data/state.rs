use std::fmt;
use std::sync::Arc;

use aether_data::repository::auth::{
    AuthApiKeyLookupKey, AuthApiKeyReadRepository, StoredAuthApiKeySnapshot,
};
use aether_data::repository::candidates::{RequestCandidateReadRepository, StoredRequestCandidate};
use aether_data::repository::provider_catalog::{
    ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use aether_data::repository::shadow_results::{
    merge_shadow_result_sample, RecordShadowResultSample, ShadowResultLookupKey,
    ShadowResultReadRepository, ShadowResultWriteRepository, StoredShadowResult,
};
use aether_data::repository::usage::{StoredRequestUsageAudit, UsageReadRepository};
use aether_data::repository::video_tasks::{
    StoredVideoTask, VideoTaskLookupKey, VideoTaskReadRepository,
};
use aether_data::{DataBackends, DataLayerError};

use super::auth::{read_auth_api_key_snapshot, StoredGatewayAuthApiKeySnapshot};
use super::candidates::{read_request_candidate_trace, RequestCandidateTrace};
use super::config::GatewayDataConfig;
use super::decision_trace::{read_decision_trace, DecisionTrace};
use super::request_audit::{read_request_audit_bundle, RequestAuditBundle};
use super::usage::{read_request_usage_audit, RequestUsageAudit};
use super::video_tasks::read_video_task_response;
use crate::gateway::video_tasks::LocalVideoTaskReadResponse;

#[derive(Clone, Default)]
pub(crate) struct GatewayDataState {
    config: GatewayDataConfig,
    backends: Option<DataBackends>,
    auth_api_key_reader: Option<Arc<dyn AuthApiKeyReadRepository>>,
    request_candidate_reader: Option<Arc<dyn RequestCandidateReadRepository>>,
    provider_catalog_reader: Option<Arc<dyn ProviderCatalogReadRepository>>,
    usage_reader: Option<Arc<dyn UsageReadRepository>>,
    video_task_reader: Option<Arc<dyn VideoTaskReadRepository>>,
    shadow_result_reader: Option<Arc<dyn ShadowResultReadRepository>>,
    shadow_result_writer: Option<Arc<dyn ShadowResultWriteRepository>>,
}

impl fmt::Debug for GatewayDataState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayDataState")
            .field("config", &self.config)
            .field("has_backends", &self.backends.is_some())
            .field(
                "has_auth_api_key_reader",
                &self.auth_api_key_reader.is_some(),
            )
            .field(
                "has_request_candidate_reader",
                &self.request_candidate_reader.is_some(),
            )
            .field(
                "has_provider_catalog_reader",
                &self.provider_catalog_reader.is_some(),
            )
            .field("has_usage_reader", &self.usage_reader.is_some())
            .field("has_video_task_reader", &self.video_task_reader.is_some())
            .field(
                "has_shadow_result_reader",
                &self.shadow_result_reader.is_some(),
            )
            .field(
                "has_shadow_result_writer",
                &self.shadow_result_writer.is_some(),
            )
            .finish()
    }
}

impl GatewayDataState {
    pub(crate) fn disabled() -> Self {
        Self::default()
    }

    pub(crate) fn from_config(config: GatewayDataConfig) -> Result<Self, DataLayerError> {
        if !config.is_enabled() {
            return Ok(Self {
                config,
                backends: None,
                auth_api_key_reader: None,
                request_candidate_reader: None,
                provider_catalog_reader: None,
                usage_reader: None,
                video_task_reader: None,
                shadow_result_reader: None,
                shadow_result_writer: None,
            });
        }

        let backends = DataBackends::from_config(config.to_data_layer_config())?;
        let auth_api_key_reader = backends.read().auth_api_keys();
        let request_candidate_reader = backends.read().request_candidates();
        let provider_catalog_reader = backends.read().provider_catalog();
        let usage_reader = backends.read().usage();
        let video_task_reader = backends.read().video_tasks();
        let shadow_result_reader = backends.read().shadow_results();
        let shadow_result_writer = backends.write().shadow_results();

        Ok(Self {
            config,
            backends: Some(backends),
            auth_api_key_reader,
            request_candidate_reader,
            provider_catalog_reader,
            usage_reader,
            video_task_reader,
            shadow_result_reader,
            shadow_result_writer,
        })
    }

    pub(crate) fn has_backends(&self) -> bool {
        self.backends.is_some()
    }

    pub(crate) fn has_auth_api_key_reader(&self) -> bool {
        self.auth_api_key_reader.is_some()
    }

    pub(crate) fn has_request_candidate_reader(&self) -> bool {
        self.request_candidate_reader.is_some()
    }

    pub(crate) fn has_provider_catalog_reader(&self) -> bool {
        self.provider_catalog_reader.is_some()
    }

    pub(crate) fn has_usage_reader(&self) -> bool {
        self.usage_reader.is_some()
    }

    pub(crate) fn has_video_task_reader(&self) -> bool {
        self.video_task_reader.is_some()
    }

    pub(crate) fn has_shadow_result_writer(&self) -> bool {
        self.shadow_result_writer.is_some()
    }

    pub(crate) fn has_shadow_result_reader(&self) -> bool {
        self.shadow_result_reader.is_some()
    }

    pub(super) async fn find_video_task(
        &self,
        key: VideoTaskLookupKey<'_>,
    ) -> Result<Option<StoredVideoTask>, DataLayerError> {
        match &self.video_task_reader {
            Some(repository) => repository.find(key).await,
            None => Ok(None),
        }
    }

    pub(super) async fn find_auth_api_key_snapshot(
        &self,
        key: AuthApiKeyLookupKey<'_>,
    ) -> Result<Option<StoredAuthApiKeySnapshot>, DataLayerError> {
        match &self.auth_api_key_reader {
            Some(repository) => repository.find_api_key_snapshot(key).await,
            None => Ok(None),
        }
    }

    pub(super) async fn list_request_candidates_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, DataLayerError> {
        match &self.request_candidate_reader {
            Some(repository) => repository.list_by_request_id(request_id).await,
            None => Ok(Vec::new()),
        }
    }

    pub(super) async fn list_provider_catalog_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, DataLayerError> {
        match &self.provider_catalog_reader {
            Some(repository) => repository.list_providers_by_ids(provider_ids).await,
            None => Ok(Vec::new()),
        }
    }

    pub(super) async fn list_provider_catalog_endpoints_by_ids(
        &self,
        endpoint_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogEndpoint>, DataLayerError> {
        match &self.provider_catalog_reader {
            Some(repository) => repository.list_endpoints_by_ids(endpoint_ids).await,
            None => Ok(Vec::new()),
        }
    }

    pub(super) async fn list_provider_catalog_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, DataLayerError> {
        match &self.provider_catalog_reader {
            Some(repository) => repository.list_keys_by_ids(key_ids).await,
            None => Ok(Vec::new()),
        }
    }

    pub(super) async fn find_request_usage_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<StoredRequestUsageAudit>, DataLayerError> {
        match &self.usage_reader {
            Some(repository) => repository.find_by_request_id(request_id).await,
            None => Ok(None),
        }
    }

    pub(crate) async fn read_request_candidate_trace(
        &self,
        request_id: &str,
        attempted_only: bool,
    ) -> Result<Option<RequestCandidateTrace>, DataLayerError> {
        read_request_candidate_trace(self, request_id, attempted_only).await
    }

    pub(crate) async fn read_decision_trace(
        &self,
        request_id: &str,
        attempted_only: bool,
    ) -> Result<Option<DecisionTrace>, DataLayerError> {
        read_decision_trace(self, request_id, attempted_only).await
    }

    pub(crate) async fn read_request_usage_audit(
        &self,
        request_id: &str,
    ) -> Result<Option<RequestUsageAudit>, DataLayerError> {
        read_request_usage_audit(self, request_id).await
    }

    pub(crate) async fn read_request_audit_bundle(
        &self,
        request_id: &str,
        attempted_only: bool,
        now_unix_secs: u64,
    ) -> Result<Option<RequestAuditBundle>, DataLayerError> {
        read_request_audit_bundle(self, request_id, attempted_only, now_unix_secs).await
    }

    pub(crate) async fn read_auth_api_key_snapshot(
        &self,
        user_id: &str,
        api_key_id: &str,
        now_unix_secs: u64,
    ) -> Result<Option<StoredGatewayAuthApiKeySnapshot>, DataLayerError> {
        read_auth_api_key_snapshot(self, user_id, api_key_id, now_unix_secs).await
    }

    pub(crate) async fn read_video_task_response(
        &self,
        route_family: Option<&str>,
        request_path: &str,
    ) -> Result<Option<LocalVideoTaskReadResponse>, DataLayerError> {
        read_video_task_response(self, route_family, request_path).await
    }

    #[cfg(test)]
    pub(crate) async fn write_shadow_result(
        &self,
        result: aether_data::repository::shadow_results::UpsertShadowResult,
    ) -> Result<Option<StoredShadowResult>, DataLayerError> {
        match &self.shadow_result_writer {
            Some(repository) => repository.upsert(result).await.map(Some),
            None => Ok(None),
        }
    }

    pub(crate) async fn record_shadow_result_sample(
        &self,
        sample: RecordShadowResultSample,
    ) -> Result<Option<StoredShadowResult>, DataLayerError> {
        let Some(writer) = &self.shadow_result_writer else {
            return Ok(None);
        };

        let existing = match &self.shadow_result_reader {
            Some(reader) => {
                reader
                    .find(ShadowResultLookupKey::TraceFingerprint {
                        trace_id: &sample.trace_id,
                        request_fingerprint: &sample.request_fingerprint,
                    })
                    .await?
            }
            None => None,
        };
        let merged = merge_shadow_result_sample(existing.as_ref(), sample);

        writer.upsert(merged).await.map(Some)
    }

    pub(crate) async fn list_recent_shadow_results(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredShadowResult>, DataLayerError> {
        match &self.shadow_result_reader {
            Some(repository) => repository.list_recent(limit).await,
            None => Ok(Vec::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_video_task_reader_for_tests(
        repository: Arc<dyn VideoTaskReadRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: None,
            provider_catalog_reader: None,
            usage_reader: None,
            video_task_reader: Some(repository),
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_request_candidate_reader_for_tests(
        repository: Arc<dyn RequestCandidateReadRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: Some(repository),
            provider_catalog_reader: None,
            usage_reader: None,
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_usage_reader_for_tests(repository: Arc<dyn UsageReadRepository>) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: None,
            provider_catalog_reader: None,
            usage_reader: Some(repository),
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_auth_api_key_reader_for_tests(
        repository: Arc<dyn AuthApiKeyReadRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: Some(repository),
            request_candidate_reader: None,
            provider_catalog_reader: None,
            usage_reader: None,
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_decision_trace_readers_for_tests(
        request_candidate_repository: Arc<dyn RequestCandidateReadRepository>,
        provider_catalog_repository: Arc<dyn ProviderCatalogReadRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: Some(request_candidate_repository),
            provider_catalog_reader: Some(provider_catalog_repository),
            usage_reader: None,
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_request_audit_readers_for_tests(
        auth_api_key_repository: Arc<dyn AuthApiKeyReadRepository>,
        request_candidate_repository: Arc<dyn RequestCandidateReadRepository>,
        provider_catalog_repository: Arc<dyn ProviderCatalogReadRepository>,
        usage_repository: Arc<dyn UsageReadRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: Some(auth_api_key_repository),
            request_candidate_reader: Some(request_candidate_repository),
            provider_catalog_reader: Some(provider_catalog_repository),
            usage_reader: Some(usage_repository),
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shadow_result_writer_for_tests(
        repository: Arc<dyn ShadowResultWriteRepository>,
    ) -> Self {
        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: None,
            provider_catalog_reader: None,
            usage_reader: None,
            video_task_reader: None,
            shadow_result_reader: None,
            shadow_result_writer: Some(repository),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shadow_result_repository_for_tests<T>(repository: Arc<T>) -> Self
    where
        T: aether_data::repository::shadow_results::ShadowResultRepository + 'static,
    {
        let shadow_result_reader: Arc<dyn ShadowResultReadRepository> = repository.clone();
        let shadow_result_writer: Arc<dyn ShadowResultWriteRepository> = repository;

        Self {
            config: GatewayDataConfig::disabled(),
            backends: None,
            auth_api_key_reader: None,
            request_candidate_reader: None,
            provider_catalog_reader: None,
            usage_reader: None,
            video_task_reader: None,
            shadow_result_reader: Some(shadow_result_reader),
            shadow_result_writer: Some(shadow_result_writer),
        }
    }
}
