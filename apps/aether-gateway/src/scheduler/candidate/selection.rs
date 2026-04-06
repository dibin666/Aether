use std::collections::{BTreeMap, BTreeSet};

use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use aether_scheduler_core::{
    collect_selectable_candidates_from_keys,
    reorder_candidates_by_scheduler_health as reorder_candidates_by_scheduler_health_in_core,
};

use crate::data::auth::GatewayAuthApiKeySnapshot;
use crate::data::candidate_selection::{
    read_minimal_candidate_selection, MinimalCandidateSelectionRowSource,
};
use crate::scheduler::affinity::SCHEDULER_AFFINITY_TTL;
use crate::GatewayError;

use super::affinity::{
    build_scheduler_affinity_cache_key, candidate_key, remember_scheduler_affinity,
};
use super::runtime::{
    auth_snapshot_concurrency_limit_reached, is_candidate_selectable,
    read_candidate_runtime_selection_snapshot,
};
use super::{SchedulerMinimalCandidateSelectionCandidate, SchedulerRuntimeState};

pub(super) fn reorder_candidates_by_scheduler_health(
    candidates: &mut [SchedulerMinimalCandidateSelectionCandidate],
    provider_key_rpm_states: &BTreeMap<String, StoredProviderCatalogKey>,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
) {
    let affinity_key = auth_snapshot
        .map(|snapshot| snapshot.api_key_id.trim())
        .filter(|value| !value.is_empty());
    reorder_candidates_by_scheduler_health_in_core(
        candidates,
        provider_key_rpm_states,
        affinity_key,
    );
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) async fn select_minimal_candidate(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &impl SchedulerRuntimeState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Option<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    let affinity_cache_key =
        build_scheduler_affinity_cache_key(auth_snapshot, api_format, global_model_name);
    let selected = collect_selectable_candidates(
        selection_row_source,
        runtime_state,
        api_format,
        global_model_name,
        require_streaming,
        auth_snapshot,
        now_unix_secs,
    )
    .await?
    .into_iter()
    .next();
    if let Some(candidate) = selected.as_ref() {
        remember_scheduler_affinity(affinity_cache_key.as_deref(), runtime_state, candidate);
    }
    Ok(selected)
}

pub(super) async fn collect_selectable_candidates(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &impl SchedulerRuntimeState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    let mut candidates = read_minimal_candidate_selection(
        selection_row_source,
        api_format,
        global_model_name,
        require_streaming,
        auth_snapshot,
    )
    .await
    .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let runtime_snapshot =
        read_candidate_runtime_selection_snapshot(runtime_state, &candidates, now_unix_secs)
            .await?;
    reorder_candidates_by_scheduler_health(
        &mut candidates,
        &runtime_snapshot.provider_key_rpm_states,
        auth_snapshot,
    );
    let affinity_cache_key =
        build_scheduler_affinity_cache_key(auth_snapshot, api_format, global_model_name);
    let cached_affinity_target = affinity_cache_key.as_deref().and_then(|cache_key| {
        runtime_state.read_cached_scheduler_affinity_target(cache_key, SCHEDULER_AFFINITY_TTL)
    });

    if auth_snapshot_concurrency_limit_reached(auth_snapshot, &runtime_snapshot, now_unix_secs) {
        return Ok(Vec::new());
    }

    let mut selected_keys = BTreeSet::new();

    for candidate in &candidates {
        if !is_candidate_selectable(
            candidate,
            &runtime_snapshot,
            now_unix_secs,
            cached_affinity_target.as_ref(),
        ) {
            continue;
        }
        selected_keys.insert(candidate_key(candidate));
    }

    Ok(collect_selectable_candidates_from_keys(
        candidates,
        &selected_keys,
        cached_affinity_target.as_ref(),
    ))
}
