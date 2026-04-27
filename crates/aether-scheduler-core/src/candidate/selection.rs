use std::collections::{BTreeMap, BTreeSet};

use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use aether_data_contracts::DataLayerError;

use super::capability::{
    enabled_required_capabilities, requested_capability_priority_for_candidate_descriptors,
};
use super::enumeration::enumerate_minimal_candidate_selection;
use super::types::{
    BuildMinimalCandidateSelectionInput, SchedulerMinimalCandidateSelectionCandidate,
    SchedulerPriorityMode,
};

pub fn build_minimal_candidate_selection(
    input: BuildMinimalCandidateSelectionInput<'_>,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, DataLayerError> {
    let priority_mode = input.priority_mode;
    let affinity_key = input.affinity_key.map(str::to_string);
    let required_capabilities = enabled_required_capabilities(input.required_capabilities);
    let mut candidates = enumerate_minimal_candidate_selection(input)?;
    let rankables = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            crate::SchedulerRankableCandidate::from_candidate(candidate, index)
                .with_capability_priority(requested_capability_priority_for_candidate_descriptors(
                    required_capabilities.iter().copied(),
                    candidate,
                ))
                .with_affinity_hash(
                    affinity_key
                        .as_deref()
                        .map(|key| crate::candidate_affinity_hash(key, candidate)),
                )
        })
        .collect::<Vec<_>>();
    crate::apply_scheduler_candidate_ranking(
        &mut candidates,
        &rankables,
        crate::SchedulerRankingContext {
            priority_mode,
            ranking_mode: crate::SchedulerRankingMode::CacheAffinity,
            include_health: false,
            load_balance_seed: 0,
        },
    );
    Ok(candidates)
}

pub fn collect_selectable_candidates_from_keys(
    candidates: Vec<SchedulerMinimalCandidateSelectionCandidate>,
    selectable_keys: &BTreeSet<(String, String, String)>,
    cached_affinity_target: Option<&crate::SchedulerAffinityTarget>,
) -> Vec<SchedulerMinimalCandidateSelectionCandidate> {
    let mut promoted = None;
    let mut selected = Vec::with_capacity(candidates.len());
    let mut emitted_keys = BTreeSet::new();

    for candidate in candidates {
        let key = crate::candidate_key(&candidate);
        if !selectable_keys.contains(&key) || !emitted_keys.insert(key) {
            continue;
        }
        if promoted.is_none()
            && cached_affinity_target
                .is_some_and(|target| crate::matches_affinity_target(&candidate, target))
        {
            promoted = Some(candidate);
        } else {
            selected.push(candidate);
        }
    }

    if let Some(candidate) = promoted {
        selected.insert(0, candidate);
    }

    selected
}

pub fn reorder_candidates_by_scheduler_health(
    candidates: &mut [SchedulerMinimalCandidateSelectionCandidate],
    provider_key_rpm_states: &BTreeMap<String, StoredProviderCatalogKey>,
    required_capabilities: Option<&serde_json::Value>,
    affinity_key: Option<&str>,
    priority_mode: SchedulerPriorityMode,
) {
    let required_capabilities = enabled_required_capabilities(required_capabilities);
    let rankables = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            crate::SchedulerRankableCandidate::from_candidate(candidate, index)
                .with_capability_priority(requested_capability_priority_for_candidate_descriptors(
                    required_capabilities.iter().copied(),
                    candidate,
                ))
                .with_affinity_hash(
                    affinity_key.map(|key| crate::candidate_affinity_hash(key, candidate)),
                )
                .with_health(
                    provider_key_rpm_states
                        .get(&candidate.key_id)
                        .and_then(|key| {
                            crate::provider_key_health_bucket(
                                key,
                                candidate.endpoint_api_format.as_str(),
                            )
                        }),
                    candidate_provider_key_health_score(candidate, Some(provider_key_rpm_states)),
                )
        })
        .collect::<Vec<_>>();
    crate::apply_scheduler_candidate_ranking(
        candidates,
        &rankables,
        crate::SchedulerRankingContext {
            priority_mode,
            ranking_mode: crate::SchedulerRankingMode::CacheAffinity,
            include_health: true,
            load_balance_seed: 0,
        },
    );
}

fn candidate_provider_key_health_score(
    candidate: &SchedulerMinimalCandidateSelectionCandidate,
    provider_key_rpm_states: Option<&BTreeMap<String, StoredProviderCatalogKey>>,
) -> f64 {
    provider_key_rpm_states
        .and_then(|states| states.get(&candidate.key_id))
        .and_then(|key| {
            crate::effective_provider_key_health_score(key, candidate.endpoint_api_format.as_str())
        })
        .unwrap_or(1.0)
}
