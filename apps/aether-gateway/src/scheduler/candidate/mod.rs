use self::affinity::candidate_affinity_hash;
use self::selection::collect_selectable_candidates;
use super::state::SchedulerRuntimeState;

mod affinity;
mod runtime;
mod selection;

#[cfg(test)]
mod tests;

use aether_data_contracts::repository::candidate_selection::{
    StoredMinimalCandidateSelectionRow, StoredProviderModelMapping,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_data_contracts::repository::quota::StoredProviderQuotaSnapshot;
use aether_scheduler_core::{
    candidate_model_names, candidate_supports_required_capability, matches_model_mapping,
    normalize_api_format, resolve_provider_model_name, select_provider_model_name,
    SchedulerMinimalCandidateSelectionCandidate,
};
use aether_wallet::{ProviderBillingType, ProviderQuotaSnapshot};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::data::auth::GatewayAuthApiKeySnapshot;
use crate::data::candidate_selection::{
    read_global_model_names_for_required_capability, MinimalCandidateSelectionRowSource,
};
use crate::GatewayError;

#[cfg_attr(not(test), allow(dead_code))]
const SCHEDULER_AFFINITY_MAX_ENTRIES: usize = 10_000;

pub(crate) async fn list_selectable_candidates(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &impl SchedulerRuntimeState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    collect_selectable_candidates(
        selection_row_source,
        runtime_state,
        api_format,
        global_model_name,
        require_streaming,
        auth_snapshot,
        now_unix_secs,
    )
    .await
}

pub(crate) async fn list_selectable_candidates_for_required_capability_without_requested_model(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &impl SchedulerRuntimeState,
    candidate_api_format: &str,
    required_capability: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    let normalized_api_format = normalize_api_format(candidate_api_format);
    if normalized_api_format.is_empty() {
        return Ok(Vec::new());
    }

    let model_names = read_global_model_names_for_required_capability(
        selection_row_source,
        &normalized_api_format,
        required_capability,
        require_streaming,
        auth_snapshot,
    )
    .await
    .map_err(|err| GatewayError::Internal(err.to_string()))?;

    for global_model_name in model_names {
        let candidates = list_selectable_candidates(
            selection_row_source,
            runtime_state,
            &normalized_api_format,
            &global_model_name,
            require_streaming,
            auth_snapshot,
            now_unix_secs,
        )
        .await?;
        let filtered = candidates
            .into_iter()
            .filter(|candidate| {
                candidate_supports_required_capability(candidate, required_capability)
            })
            .collect::<Vec<_>>();
        if !filtered.is_empty() {
            return Ok(filtered);
        }
    }

    Ok(Vec::new())
}
