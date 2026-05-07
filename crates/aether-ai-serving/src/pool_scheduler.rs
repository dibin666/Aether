use std::cmp::Ordering;
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

pub const AI_POOL_ACCOUNT_BLOCKED_SKIP_REASON: &str = "pool_account_blocked";
pub const AI_POOL_ACCOUNT_EXHAUSTED_SKIP_REASON: &str = "pool_account_exhausted";
pub const AI_POOL_PLAN_NOT_SELECTED_SKIP_REASON: &str = "pool_plan_not_selected";
pub const AI_POOL_COOLDOWN_SKIP_REASON: &str = "pool_cooldown";
pub const AI_POOL_COST_LIMIT_REACHED_SKIP_REASON: &str = "pool_cost_limit_reached";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPoolSchedulingPreset {
    pub preset: String,
    pub enabled: bool,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPoolSchedulingConfig {
    pub scheduling_presets: Vec<AiPoolSchedulingPreset>,
    pub lru_enabled: bool,
    pub skip_exhausted_accounts: bool,
    pub cost_limit_per_key_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiPoolRuntimeState {
    pub sticky_bound_key_id: Option<String>,
    pub cooldown_reason_by_key: BTreeMap<String, String>,
    pub cost_window_usage_by_key: BTreeMap<String, u64>,
    pub latency_avg_ms_by_key: BTreeMap<String, f64>,
    pub lru_score_by_key: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiPoolCatalogKeyContext {
    pub oauth_plan_type: Option<String>,
    pub quota_usage_ratio: Option<f64>,
    pub quota_reset_seconds: Option<f64>,
    pub account_blocked: bool,
    pub quota_exhausted: bool,
    pub health_score: Option<f64>,
    pub latency_avg_ms: Option<f64>,
    pub catalog_lru_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPoolCandidateFacts {
    pub provider_id: String,
    pub endpoint_id: String,
    pub model_id: String,
    pub selected_provider_model_name: String,
    pub provider_api_format: String,
    pub provider_type: String,
    pub key_id: String,
    pub key_internal_priority: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiPoolCandidateOrchestration {
    pub candidate_group_id: Option<String>,
    pub pool_key_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiPoolCandidateInput<Candidate> {
    pub candidate: Candidate,
    pub facts: AiPoolCandidateFacts,
    pub pool_config: Option<AiPoolSchedulingConfig>,
    pub key_context: AiPoolCatalogKeyContext,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiPoolScheduledCandidate<Candidate> {
    pub candidate: Candidate,
    pub orchestration: AiPoolCandidateOrchestration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiPoolSkippedCandidate<Candidate> {
    pub candidate: Candidate,
    pub skip_reason: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiPoolSchedulerOutcome<Candidate> {
    pub candidates: Vec<AiPoolScheduledCandidate<Candidate>>,
    pub skipped_candidates: Vec<AiPoolSkippedCandidate<Candidate>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PoolGroupKey {
    provider_id: String,
    endpoint_id: String,
    model_id: String,
    selected_provider_model_name: String,
    provider_api_format: String,
    singleton_key_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedPoolPreset {
    preset: String,
    mode: Option<String>,
    auto_added: bool,
}

pub fn run_ai_pool_scheduler<Candidate>(
    candidates: Vec<AiPoolCandidateInput<Candidate>>,
    runtime_by_provider: &BTreeMap<String, AiPoolRuntimeState>,
    load_balance_seed_nonce: &str,
) -> AiPoolSchedulerOutcome<Candidate> {
    let mut group_order = Vec::new();
    let mut groups = BTreeMap::<PoolGroupKey, Vec<AiPoolCandidateInput<Candidate>>>::new();

    for candidate in candidates {
        let pool_enabled = candidate.pool_config.is_some();
        let group_key = pool_group_key(&candidate, pool_enabled);
        match groups.entry(group_key) {
            Entry::Vacant(entry) => {
                group_order.push(entry.key().clone());
                entry.insert(vec![candidate]);
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(candidate);
            }
        }
    }

    let mut reordered = Vec::new();
    let mut skipped = Vec::new();
    let default_runtime = AiPoolRuntimeState::default();

    for group_key in group_order {
        let Some(group) = groups.remove(&group_key) else {
            continue;
        };
        let candidate_group_id = ai_pool_candidate_group_id(&group_key);
        let Some(pool_config) = group
            .first()
            .expect("group should exist")
            .pool_config
            .clone()
        else {
            reordered.extend(annotate_ai_pool_candidates(
                group,
                candidate_group_id.as_str(),
                false,
            ));
            continue;
        };
        let runtime = runtime_by_provider
            .get(&group_key.provider_id)
            .unwrap_or(&default_runtime);
        let outcome = schedule_pool_group(
            group,
            &pool_config,
            runtime,
            candidate_group_id.as_str(),
            load_balance_seed_nonce,
        );
        reordered.extend(outcome.candidates);
        skipped.extend(outcome.skipped_candidates);
    }

    AiPoolSchedulerOutcome {
        candidates: reordered,
        skipped_candidates: skipped,
    }
}

fn pool_group_key<Candidate>(
    candidate: &AiPoolCandidateInput<Candidate>,
    pool_enabled: bool,
) -> PoolGroupKey {
    PoolGroupKey {
        provider_id: candidate.facts.provider_id.clone(),
        endpoint_id: candidate.facts.endpoint_id.clone(),
        model_id: candidate.facts.model_id.clone(),
        selected_provider_model_name: candidate.facts.selected_provider_model_name.clone(),
        provider_api_format: candidate.facts.provider_api_format.clone(),
        singleton_key_id: (!pool_enabled).then(|| candidate.facts.key_id.clone()),
    }
}

fn ai_pool_candidate_group_id(group_key: &PoolGroupKey) -> String {
    format!(
        "provider={}|endpoint={}|model={}|selected_model={}|api_format={}|singleton_key={}",
        group_key.provider_id,
        group_key.endpoint_id,
        group_key.model_id,
        group_key.selected_provider_model_name,
        group_key.provider_api_format,
        group_key.singleton_key_id.as_deref().unwrap_or("*"),
    )
}

fn schedule_pool_group<Candidate>(
    group: Vec<AiPoolCandidateInput<Candidate>>,
    pool_config: &AiPoolSchedulingConfig,
    runtime: &AiPoolRuntimeState,
    candidate_group_id: &str,
    load_balance_seed_nonce: &str,
) -> AiPoolSchedulerOutcome<Candidate> {
    let provider_type = group
        .first()
        .map(|candidate| candidate.facts.provider_type.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let active_presets =
        normalize_enabled_pool_presets(&pool_config.scheduling_presets, provider_type.as_str());
    let selected_plan_order = selected_pool_plan_order(&active_presets);

    let mut available = Vec::new();
    let mut skipped = Vec::new();

    for (original_index, mut item) in group.into_iter().enumerate() {
        let key_id = item.facts.key_id.clone();
        item.key_context.latency_avg_ms = runtime
            .latency_avg_ms_by_key
            .get(&key_id)
            .copied()
            .or(item.key_context.latency_avg_ms);

        if item.key_context.account_blocked {
            skipped.push(AiPoolSkippedCandidate {
                candidate: item.candidate,
                skip_reason: AI_POOL_ACCOUNT_BLOCKED_SKIP_REASON,
            });
            continue;
        }

        if pool_config.skip_exhausted_accounts && item.key_context.quota_exhausted {
            skipped.push(AiPoolSkippedCandidate {
                candidate: item.candidate,
                skip_reason: AI_POOL_ACCOUNT_EXHAUSTED_SKIP_REASON,
            });
            continue;
        }

        if let Some(selected_plan_order) = selected_plan_order.as_ref() {
            let plan_type = item
                .key_context
                .oauth_plan_type
                .as_deref()
                .unwrap_or_default();
            if !selected_plan_order.contains_key(plan_type) {
                skipped.push(AiPoolSkippedCandidate {
                    candidate: item.candidate,
                    skip_reason: AI_POOL_PLAN_NOT_SELECTED_SKIP_REASON,
                });
                continue;
            }
        }

        if runtime.cooldown_reason_by_key.contains_key(&key_id) {
            skipped.push(AiPoolSkippedCandidate {
                candidate: item.candidate,
                skip_reason: AI_POOL_COOLDOWN_SKIP_REASON,
            });
            continue;
        }

        if pool_config
            .cost_limit_per_key_tokens
            .is_some_and(|limit| runtime_cost_usage(runtime, key_id.as_str()) >= limit)
        {
            skipped.push(AiPoolSkippedCandidate {
                candidate: item.candidate,
                skip_reason: AI_POOL_COST_LIMIT_REACHED_SKIP_REASON,
            });
            continue;
        }

        let lru_score =
            runtime_lru_score(runtime, key_id.as_str()).or(item.key_context.catalog_lru_score);

        available.push(PoolGroupCandidateOrdering {
            item,
            original_index,
            lru_score,
            cost_usage: runtime_cost_usage(runtime, key_id.as_str()),
        });
    }

    if available.is_empty() {
        return AiPoolSchedulerOutcome {
            candidates: Vec::new(),
            skipped_candidates: skipped,
        };
    }

    if !active_presets.is_empty() {
        let sort_vectors = build_pool_sort_vectors(
            &available,
            &active_presets,
            selected_plan_order.as_ref(),
            pool_config.lru_enabled,
            group_sort_seed(
                provider_type.as_str(),
                available.first().map(|item| &item.item.facts),
                load_balance_seed_nonce,
            )
            .as_str(),
            pool_config.cost_limit_per_key_tokens,
        );
        available.sort_by(|left, right| {
            sort_vectors
                .get(&left.item.facts.key_id)
                .cmp(&sort_vectors.get(&right.item.facts.key_id))
                .then(left.original_index.cmp(&right.original_index))
        });
        let primary_strategy_components = usize::from(selected_plan_order.is_some());
        promote_sticky_candidate(
            &mut available,
            runtime.sticky_bound_key_id.as_deref(),
            Some(&sort_vectors),
            primary_strategy_components,
        );
    } else {
        if pool_config.lru_enabled {
            let lru_ranks = lru_rank_indices(&available, false);
            available.sort_by(|left, right| {
                lru_ranks
                    .get(&left.item.facts.key_id)
                    .cmp(&lru_ranks.get(&right.item.facts.key_id))
                    .then(left.original_index.cmp(&right.original_index))
            });
        }
        promote_sticky_candidate(
            &mut available,
            runtime.sticky_bound_key_id.as_deref(),
            None,
            0,
        );
    }

    let ordered = available
        .into_iter()
        .map(|item| item.item)
        .collect::<Vec<_>>();

    AiPoolSchedulerOutcome {
        candidates: annotate_ai_pool_candidates(ordered, candidate_group_id, true),
        skipped_candidates: skipped,
    }
}

fn annotate_ai_pool_candidates<Candidate>(
    candidates: Vec<AiPoolCandidateInput<Candidate>>,
    candidate_group_id: &str,
    pool_enabled: bool,
) -> Vec<AiPoolScheduledCandidate<Candidate>> {
    candidates
        .into_iter()
        .enumerate()
        .map(|(index, item)| AiPoolScheduledCandidate {
            candidate: item.candidate,
            orchestration: AiPoolCandidateOrchestration {
                candidate_group_id: Some(candidate_group_id.to_string()),
                pool_key_index: pool_enabled.then_some(index as u32),
            },
        })
        .collect()
}

#[derive(Debug)]
struct PoolGroupCandidateOrdering<Candidate> {
    item: AiPoolCandidateInput<Candidate>,
    original_index: usize,
    lru_score: Option<f64>,
    cost_usage: u64,
}

fn selected_pool_plan_order(
    presets: &[NormalizedPoolPreset],
) -> Option<BTreeMap<&'static str, usize>> {
    let mut selected = BTreeMap::new();
    for preset in presets {
        let Some(plan_type) = pool_plan_type_for_priority_preset(&preset.preset) else {
            continue;
        };
        if !selected.contains_key(plan_type) {
            let rank = selected.len();
            selected.insert(plan_type, rank);
        }
    }
    (!selected.is_empty()).then_some(selected)
}

fn pool_plan_type_for_priority_preset(preset: &str) -> Option<&'static str> {
    match preset {
        "free_first" => Some("free"),
        "team_first" => Some("team"),
        "plus_first" => Some("plus"),
        "pro_first" => Some("pro"),
        _ => None,
    }
}

fn build_pool_sort_vectors<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    presets: &[NormalizedPoolPreset],
    selected_plan_order: Option<&BTreeMap<&'static str, usize>>,
    lru_enabled: bool,
    load_balance_seed: &str,
    cost_limit_per_key_tokens: Option<u64>,
) -> BTreeMap<String, Vec<usize>> {
    let mut vectors = BTreeMap::<String, Vec<usize>>::new();
    let lru_ranks = lru_rank_indices(items, false);
    let cache_affinity_ranks = lru_rank_indices(items, true);

    let mut plan_order_added = false;
    for preset in presets {
        let ranks = if pool_plan_type_for_priority_preset(&preset.preset).is_some() {
            if plan_order_added {
                continue;
            }
            plan_order_added = true;
            let Some(selected_plan_order) = selected_plan_order else {
                continue;
            };
            plan_order_ranks(items, &lru_ranks, selected_plan_order)
        } else {
            match preset.preset.as_str() {
                "cache_affinity" => cache_affinity_ranks.clone(),
                "priority_first" => priority_first_ranks(items, &lru_ranks),
                "single_account" => single_account_ranks(items),
                "health_first" => health_first_ranks(items, &lru_ranks),
                "latency_first" => latency_first_ranks(items, &lru_ranks),
                "cost_first" => cost_first_ranks(items, &lru_ranks, cost_limit_per_key_tokens),
                "quota_balanced" => {
                    quota_balanced_ranks(items, &lru_ranks, cost_limit_per_key_tokens)
                }
                "recent_refresh" => recent_refresh_ranks(items, &lru_ranks),
                "load_balance" => load_balance_ranks(items, load_balance_seed),
                _ => continue,
            }
        };
        for item in items {
            let key_id = item.item.facts.key_id.clone();
            vectors
                .entry(key_id.clone())
                .or_default()
                .push(*ranks.get(&key_id).unwrap_or(&0));
        }
    }

    if lru_enabled {
        for item in items {
            let key_id = item.item.facts.key_id.clone();
            vectors
                .entry(key_id.clone())
                .or_default()
                .push(*lru_ranks.get(&key_id).unwrap_or(&0));
        }
    }

    vectors
}

fn lru_rank_indices<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    descending: bool,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| item.lru_score);
    rank_indices_from_score_map(items, &scores, descending)
}

fn priority_first_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| {
        Some(f64::from(item.item.facts.key_internal_priority))
    });
    if !score_map_has_variation(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn single_account_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
) -> BTreeMap<String, usize> {
    let n = items.len().saturating_sub(1).max(1) as f64;
    let priority_scores = collect_metric_scores(items, |item| {
        Some(f64::from(item.item.facts.key_internal_priority))
    });
    let priority_ranks = rank_indices_from_score_map(items, &priority_scores, false);
    let lru_desc_ranks = lru_rank_indices(items, true);
    let combined_scores = items
        .iter()
        .map(|item| {
            let key_id = item.item.facts.key_id.clone();
            let priority_rank = *priority_ranks.get(&key_id).unwrap_or(&0) as f64 / n;
            let lru_rank = *lru_desc_ranks.get(&key_id).unwrap_or(&0) as f64 / n;
            (key_id, Some((priority_rank * 0.75) + (lru_rank * 0.25)))
        })
        .collect::<BTreeMap<_, _>>();
    rank_indices_from_score_map(items, &combined_scores, false)
}

fn plan_order_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
    selected_plan_order: &BTreeMap<&'static str, usize>,
) -> BTreeMap<String, usize> {
    let fallback_rank = selected_plan_order.len() as f64;
    let scores = items
        .iter()
        .map(|item| {
            let plan_rank = item
                .item
                .key_context
                .oauth_plan_type
                .as_deref()
                .and_then(|plan_type| selected_plan_order.get(plan_type).copied())
                .map(|rank| rank as f64)
                .unwrap_or(fallback_rank);
            (item.item.facts.key_id.clone(), Some(plan_rank))
        })
        .collect::<BTreeMap<_, _>>();
    if !score_map_has_variation(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn neutral_pool_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
) -> BTreeMap<String, usize> {
    items
        .iter()
        .map(|item| (item.item.facts.key_id.clone(), 0))
        .collect()
}

fn health_first_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| {
        item.item
            .key_context
            .health_score
            .map(|score| 1.0 - score.clamp(0.0, 1.0))
    });
    if !score_map_has_signal(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn latency_first_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| item.item.key_context.latency_avg_ms);
    if !score_map_has_signal(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn cost_first_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
    cost_limit_per_key_tokens: Option<u64>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| {
        cost_penalty(item, cost_limit_per_key_tokens).or(item.item.key_context.quota_usage_ratio)
    });
    if !score_map_has_signal(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn quota_balanced_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
    cost_limit_per_key_tokens: Option<u64>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| {
        item.item
            .key_context
            .quota_usage_ratio
            .or_else(|| cost_penalty(item, cost_limit_per_key_tokens))
    });
    if !score_map_has_signal(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn recent_refresh_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    _lru_ranks: &BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    let scores = collect_metric_scores(items, |item| item.item.key_context.quota_reset_seconds);
    if !score_map_has_signal(&scores) {
        return neutral_pool_ranks(items);
    }
    rank_indices_from_score_map(items, &scores, false)
}

fn load_balance_ranks<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    load_balance_seed: &str,
) -> BTreeMap<String, usize> {
    let scores = items
        .iter()
        .map(|item| {
            let key_id = item.item.facts.key_id.clone();
            (
                key_id.clone(),
                Some(stable_hash_score(
                    format!("{load_balance_seed}:{key_id}").as_str(),
                )),
            )
        })
        .collect::<BTreeMap<_, _>>();
    rank_indices_from_score_map(items, &scores, false)
}

fn group_sort_seed(
    provider_type: &str,
    candidate: Option<&AiPoolCandidateFacts>,
    load_balance_seed_nonce: &str,
) -> String {
    match candidate {
        Some(candidate) => format!(
            "{provider_type}:{}:{}:{}:{}:{load_balance_seed_nonce}",
            candidate.provider_id,
            candidate.endpoint_id,
            candidate.model_id,
            candidate.selected_provider_model_name,
        ),
        None => format!("{provider_type}:{load_balance_seed_nonce}"),
    }
}

fn stable_hash_score(seed: &str) -> f64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let value = hasher.finish();
    value as f64 / u64::MAX as f64
}

fn collect_metric_scores<Candidate, F>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    mut score_for: F,
) -> BTreeMap<String, Option<f64>>
where
    F: FnMut(&PoolGroupCandidateOrdering<Candidate>) -> Option<f64>,
{
    items
        .iter()
        .map(|item| (item.item.facts.key_id.clone(), score_for(item)))
        .collect()
}

fn score_map_has_signal(scores: &BTreeMap<String, Option<f64>>) -> bool {
    scores.values().flatten().any(|value| value.is_finite())
}

fn score_map_has_variation(scores: &BTreeMap<String, Option<f64>>) -> bool {
    let values = scores
        .values()
        .flatten()
        .filter(|value| value.is_finite())
        .map(|value| value.to_bits())
        .collect::<BTreeSet<_>>();
    values.len() > 1
}

fn rank_indices_from_score_map<Candidate>(
    items: &[PoolGroupCandidateOrdering<Candidate>],
    scores: &BTreeMap<String, Option<f64>>,
    descending: bool,
) -> BTreeMap<String, usize> {
    if !score_map_has_signal(scores) {
        return items
            .iter()
            .map(|item| (item.item.facts.key_id.clone(), 0))
            .collect();
    }

    let mut decorated = items
        .iter()
        .map(|item| {
            let key_id = item.item.facts.key_id.clone();
            let score = scores
                .get(&key_id)
                .copied()
                .flatten()
                .filter(|value| value.is_finite());
            let sortable = score.map(|value| if descending { -value } else { value });
            (
                score.is_none(),
                sortable.unwrap_or(f64::INFINITY),
                item.original_index,
                key_id,
            )
        })
        .collect::<Vec<_>>();
    decorated.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.partial_cmp(&right.1).unwrap_or(Ordering::Equal))
            .then(left.2.cmp(&right.2))
    });

    let mut ranks = BTreeMap::new();
    let mut current_rank = 0usize;
    let mut previous_rank_key = None::<(bool, f64)>;
    for (index, (missing, sortable, _, key_id)) in decorated.into_iter().enumerate() {
        if let Some((previous_missing, previous_sortable)) = previous_rank_key {
            if previous_missing != missing
                || previous_sortable.partial_cmp(&sortable) != Some(Ordering::Equal)
            {
                current_rank = index;
            }
        }
        previous_rank_key = Some((missing, sortable));
        ranks.insert(key_id, current_rank);
    }
    ranks
}

fn cost_penalty<Candidate>(
    item: &PoolGroupCandidateOrdering<Candidate>,
    cost_limit_per_key_tokens: Option<u64>,
) -> Option<f64> {
    if item.cost_usage == 0 {
        return None;
    }

    if let Some(limit) = cost_limit_per_key_tokens.filter(|limit| *limit > 0) {
        return Some((item.cost_usage as f64 / limit as f64).clamp(0.0, 1.0));
    }

    let used = item.cost_usage as f64;
    Some((used / (used + 10_000.0)).clamp(0.0, 1.0))
}

pub fn normalize_enabled_ai_pool_presets(
    scheduling_presets: &[AiPoolSchedulingPreset],
    provider_type: &str,
) -> Vec<String> {
    normalize_enabled_pool_presets(scheduling_presets, provider_type)
        .into_iter()
        .map(|preset| preset.preset)
        .collect()
}

fn normalize_enabled_pool_presets(
    scheduling_presets: &[AiPoolSchedulingPreset],
    provider_type: &str,
) -> Vec<NormalizedPoolPreset> {
    let provider_type = provider_type.trim().to_ascii_lowercase();
    let mut entries = Vec::<(usize, String, bool, Option<String>, bool)>::new();
    let mut seen = BTreeSet::new();

    for (index, item) in scheduling_presets.iter().enumerate() {
        let preset = item.preset.trim().to_ascii_lowercase();
        if preset.is_empty() || !seen.insert(preset.clone()) {
            continue;
        }
        entries.push((index, preset, item.enabled, item.mode.clone(), false));
    }

    if provider_type == "codex"
        && !entries.is_empty()
        && entries
            .iter()
            .all(|(_, preset, _, _, _)| preset != "recent_refresh")
    {
        entries.push((
            entries.len(),
            "recent_refresh".to_string(),
            true,
            None,
            true,
        ));
    }

    let mut group_anchor_index = BTreeMap::<String, usize>::new();
    for (index, preset, _, _, _) in &entries {
        let Some(mutex_group) = pool_preset_mutex_group(preset) else {
            continue;
        };
        group_anchor_index
            .entry(mutex_group.to_string())
            .or_insert(*index);
    }

    let mut ordered_enabled = Vec::<(usize, usize, String, Option<String>, bool)>::new();
    let mut group_enabled = BTreeMap::<String, (usize, usize, String, Option<String>, bool)>::new();

    for (index, preset, enabled, mode, auto_added) in entries {
        if !enabled
            || preset == "lru"
            || !pool_preset_supported_for_provider(&preset, &provider_type)
        {
            continue;
        }

        let Some(mutex_group) = pool_preset_mutex_group(&preset) else {
            ordered_enabled.push((index, index, preset, mode, auto_added));
            continue;
        };
        let anchor = group_anchor_index
            .get(mutex_group)
            .copied()
            .unwrap_or(index);
        let existing = group_enabled.get(mutex_group);
        if existing.is_none_or(|current| index < current.1) {
            group_enabled.insert(
                mutex_group.to_string(),
                (anchor, index, preset, mode, auto_added),
            );
        }
    }

    ordered_enabled.extend(group_enabled.into_values());
    ordered_enabled.sort_by(|left, right| {
        pool_preset_order_tier(&left.2, left.4)
            .cmp(&pool_preset_order_tier(&right.2, right.4))
            .then(left.0.cmp(&right.0))
            .then(left.1.cmp(&right.1))
    });
    ordered_enabled
        .into_iter()
        .map(|(_, _, preset, mode, auto_added)| NormalizedPoolPreset {
            preset,
            mode,
            auto_added,
        })
        .collect()
}

fn pool_preset_supported_for_provider(preset: &str, provider_type: &str) -> bool {
    match preset {
        "free_first" | "plus_first" | "pro_first" | "recent_refresh" | "team_first" => {
            matches!(provider_type, "codex" | "kiro")
        }
        _ => true,
    }
}

fn pool_preset_mutex_group(preset: &str) -> Option<&'static str> {
    match preset {
        "lru" | "cache_affinity" | "load_balance" | "single_account" => Some("distribution_mode"),
        _ => None,
    }
}

fn pool_preset_order_tier(preset: &str, auto_added: bool) -> usize {
    if auto_added {
        3
    } else if pool_preset_mutex_group(preset).is_some() {
        1
    } else {
        0
    }
}

fn promote_sticky_candidate<Candidate>(
    available: &mut Vec<PoolGroupCandidateOrdering<Candidate>>,
    sticky_key_id: Option<&str>,
    sort_vectors: Option<&BTreeMap<String, Vec<usize>>>,
    primary_strategy_components: usize,
) {
    let Some(sticky_key_id) = sticky_key_id else {
        return;
    };
    let Some(sticky_index) = available
        .iter()
        .position(|item| item.item.facts.key_id == sticky_key_id)
    else {
        return;
    };

    let target_index = if primary_strategy_components == 0 {
        0
    } else {
        let Some(sort_vectors) = sort_vectors else {
            return;
        };
        let Some(sticky_prefix) = sort_vectors.get(sticky_key_id).map(|vector| {
            let end = vector.len().min(primary_strategy_components);
            &vector[..end]
        }) else {
            return;
        };
        available
            .iter()
            .position(|item| {
                sort_vectors
                    .get(&item.item.facts.key_id)
                    .map(|vector| {
                        let end = vector.len().min(primary_strategy_components);
                        &vector[..end] == sticky_prefix
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(sticky_index)
    };

    if target_index < sticky_index {
        let sticky_candidate = available.remove(sticky_index);
        available.insert(target_index, sticky_candidate);
    }
}

fn runtime_lru_score(runtime: &AiPoolRuntimeState, key_id: &str) -> Option<f64> {
    runtime.lru_score_by_key.get(key_id).copied()
}

fn runtime_cost_usage(runtime: &AiPoolRuntimeState, key_id: &str) -> u64 {
    runtime
        .cost_window_usage_by_key
        .get(key_id)
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_scheduler_groups_interleaved_candidates_and_reorders_internal_keys() {
        let pool_first = sample_candidate("provider-pool", "endpoint-1", "key-pool-a", 10, true);
        let other = sample_candidate("provider-other", "endpoint-2", "key-other", 10, false);
        let pool_second = sample_candidate("provider-pool", "endpoint-1", "key-pool-b", 10, true);

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                lru_score_by_key: BTreeMap::from([
                    ("key-pool-a".to_string(), 20.0),
                    ("key-pool-b".to_string(), 10.0),
                ]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(
            vec![pool_first, other, pool_second],
            &runtime_by_provider,
            "seed",
        );

        assert!(outcome.skipped_candidates.is_empty());
        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-pool-b", "key-pool-a", "key-other"]
        );
    }

    #[test]
    fn pool_scheduler_skips_cooldown_and_cost_exhausted_keys() {
        let key_ready = sample_candidate("provider-pool", "endpoint-1", "key-ready", 10, true)
            .with_cost_limit(100);
        let key_cooldown =
            sample_candidate("provider-pool", "endpoint-1", "key-cooldown", 10, true)
                .with_cost_limit(100);
        let key_cost = sample_candidate("provider-pool", "endpoint-1", "key-cost", 10, true)
            .with_cost_limit(100);

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                cooldown_reason_by_key: BTreeMap::from([(
                    "key-cooldown".to_string(),
                    "429".to_string(),
                )]),
                cost_window_usage_by_key: BTreeMap::from([("key-cost".to_string(), 100)]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(
            vec![key_ready, key_cooldown, key_cost],
            &runtime_by_provider,
            "seed",
        );

        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-ready"]
        );
        assert_eq!(
            outcome
                .skipped_candidates
                .iter()
                .map(|item| (item.candidate.as_str(), item.skip_reason))
                .collect::<Vec<_>>(),
            vec![
                ("key-cooldown", "pool_cooldown"),
                ("key-cost", "pool_cost_limit_reached"),
            ]
        );
    }

    #[test]
    fn pool_scheduler_promotes_sticky_hit_before_other_sorted_keys() {
        let key_a = sample_candidate("provider-pool", "endpoint-1", "key-a", 10, true);
        let key_b = sample_candidate("provider-pool", "endpoint-1", "key-b", 10, true);

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                sticky_bound_key_id: Some("key-a".to_string()),
                lru_score_by_key: BTreeMap::from([
                    ("key-a".to_string(), 50.0),
                    ("key-b".to_string(), 10.0),
                ]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(vec![key_a, key_b], &runtime_by_provider, "seed");

        assert!(outcome.skipped_candidates.is_empty());
        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-a", "key-b"]
        );
    }

    #[test]
    fn pool_scheduler_uses_plan_preset_with_catalog_context() {
        let key_free = sample_candidate("provider-pool", "endpoint-1", "key-free", 10, true)
            .with_presets(vec![AiPoolSchedulingPreset {
                preset: "plus_first".to_string(),
                enabled: true,
                mode: None,
            }])
            .with_plan("free");
        let key_plus = sample_candidate("provider-pool", "endpoint-1", "key-plus", 10, true)
            .with_presets(vec![AiPoolSchedulingPreset {
                preset: "plus_first".to_string(),
                enabled: true,
                mode: None,
            }])
            .with_plan("plus");

        let outcome = run_ai_pool_scheduler(vec![key_free, key_plus], &BTreeMap::new(), "seed");

        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-plus"]
        );
        assert_eq!(
            outcome
                .skipped_candidates
                .iter()
                .map(|item| (item.candidate.as_str(), item.skip_reason))
                .collect::<Vec<_>>(),
            vec![("key-free", "pool_plan_not_selected")]
        );
    }

    #[test]
    fn pool_scheduler_filters_unselected_plan_tiers_and_respects_selected_order() {
        let key_plus = sample_candidate("provider-pool", "endpoint-1", "key-plus", 10, true)
            .with_presets(vec![
                AiPoolSchedulingPreset {
                    preset: "plus_first".to_string(),
                    enabled: true,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "team_first".to_string(),
                    enabled: true,
                    mode: None,
                },
            ])
            .with_plan("plus");
        let key_pro = sample_candidate("provider-pool", "endpoint-1", "key-pro", 10, true)
            .with_presets(vec![
                AiPoolSchedulingPreset {
                    preset: "plus_first".to_string(),
                    enabled: true,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "team_first".to_string(),
                    enabled: true,
                    mode: None,
                },
            ])
            .with_plan("pro");
        let key_team = sample_candidate("provider-pool", "endpoint-1", "key-team", 10, true)
            .with_presets(vec![
                AiPoolSchedulingPreset {
                    preset: "plus_first".to_string(),
                    enabled: true,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "team_first".to_string(),
                    enabled: true,
                    mode: None,
                },
            ])
            .with_plan("team");

        let outcome =
            run_ai_pool_scheduler(vec![key_plus, key_pro, key_team], &BTreeMap::new(), "seed");

        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-plus", "key-team"]
        );
        assert_eq!(
            outcome
                .skipped_candidates
                .iter()
                .map(|item| (item.candidate.as_str(), item.skip_reason))
                .collect::<Vec<_>>(),
            vec![("key-pro", "pool_plan_not_selected")]
        );
    }

    #[test]
    fn pool_scheduler_plan_order_precedes_cache_affinity_distribution() {
        let presets = vec![
            AiPoolSchedulingPreset {
                preset: "cache_affinity".to_string(),
                enabled: true,
                mode: None,
            },
            AiPoolSchedulingPreset {
                preset: "plus_first".to_string(),
                enabled: true,
                mode: None,
            },
            AiPoolSchedulingPreset {
                preset: "free_first".to_string(),
                enabled: true,
                mode: None,
            },
        ];
        let key_free = sample_candidate("provider-pool", "endpoint-1", "key-free", 10, true)
            .with_presets(presets.clone())
            .with_plan("free");
        let key_plus = sample_candidate("provider-pool", "endpoint-1", "key-plus", 10, true)
            .with_presets(presets)
            .with_plan("plus");

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                lru_score_by_key: BTreeMap::from([
                    ("key-free".to_string(), 100.0),
                    ("key-plus".to_string(), 1.0),
                ]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(
            vec![key_free, key_plus],
            &runtime_by_provider,
            "seed",
        );

        assert!(outcome.skipped_candidates.is_empty());
        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-plus", "key-free"]
        );
    }

    #[test]
    fn pool_scheduler_sticky_hit_does_not_cross_plan_priority() {
        let presets = vec![
            AiPoolSchedulingPreset {
                preset: "cache_affinity".to_string(),
                enabled: true,
                mode: None,
            },
            AiPoolSchedulingPreset {
                preset: "plus_first".to_string(),
                enabled: true,
                mode: None,
            },
            AiPoolSchedulingPreset {
                preset: "free_first".to_string(),
                enabled: true,
                mode: None,
            },
        ];
        let key_plus = sample_candidate("provider-pool", "endpoint-1", "key-plus", 10, true)
            .with_presets(presets.clone())
            .with_plan("plus");
        let key_free = sample_candidate("provider-pool", "endpoint-1", "key-free", 10, true)
            .with_presets(presets)
            .with_plan("free");

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                sticky_bound_key_id: Some("key-free".to_string()),
                lru_score_by_key: BTreeMap::from([
                    ("key-free".to_string(), 100.0),
                    ("key-plus".to_string(), 1.0),
                ]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(
            vec![key_plus, key_free],
            &runtime_by_provider,
            "seed",
        );

        assert!(outcome.skipped_candidates.is_empty());
        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-plus", "key-free"]
        );
    }

    #[test]
    fn pool_scheduler_cache_affinity_breaks_ties_within_same_plan() {
        let presets = vec![
            AiPoolSchedulingPreset {
                preset: "cache_affinity".to_string(),
                enabled: true,
                mode: None,
            },
            AiPoolSchedulingPreset {
                preset: "plus_first".to_string(),
                enabled: true,
                mode: None,
            },
        ];
        let key_a = sample_candidate("provider-pool", "endpoint-1", "key-plus-a", 10, true)
            .with_presets(presets.clone())
            .with_plan("plus");
        let key_b = sample_candidate("provider-pool", "endpoint-1", "key-plus-b", 10, true)
            .with_presets(presets)
            .with_plan("plus");

        let runtime_by_provider = BTreeMap::from([(
            "provider-pool".to_string(),
            AiPoolRuntimeState {
                lru_score_by_key: BTreeMap::from([
                    ("key-plus-a".to_string(), 1.0),
                    ("key-plus-b".to_string(), 100.0),
                ]),
                ..AiPoolRuntimeState::default()
            },
        )]);

        let outcome = run_ai_pool_scheduler(vec![key_a, key_b], &runtime_by_provider, "seed");

        assert!(outcome.skipped_candidates.is_empty());
        assert_eq!(
            outcome
                .candidates
                .iter()
                .map(|item| item.candidate.as_str())
                .collect::<Vec<_>>(),
            vec!["key-plus-b", "key-plus-a"]
        );
    }

    #[test]
    fn normalizes_distribution_mutex_group_to_first_enabled_member() {
        let presets = normalize_enabled_ai_pool_presets(
            &[
                AiPoolSchedulingPreset {
                    preset: "lru".to_string(),
                    enabled: false,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "single_account".to_string(),
                    enabled: true,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "cache_affinity".to_string(),
                    enabled: true,
                    mode: None,
                },
                AiPoolSchedulingPreset {
                    preset: "priority_first".to_string(),
                    enabled: true,
                    mode: None,
                },
            ],
            "openai",
        );

        assert_eq!(presets, ["priority_first", "single_account"]);
    }

    fn sample_candidate(
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
        internal_priority: i32,
        pool_enabled: bool,
    ) -> AiPoolCandidateInput<String> {
        let pool_config = pool_enabled.then(|| AiPoolSchedulingConfig {
            scheduling_presets: Vec::new(),
            lru_enabled: true,
            skip_exhausted_accounts: false,
            cost_limit_per_key_tokens: None,
        });
        AiPoolCandidateInput {
            candidate: key_id.to_string(),
            facts: AiPoolCandidateFacts {
                provider_id: provider_id.to_string(),
                endpoint_id: endpoint_id.to_string(),
                model_id: "model-1".to_string(),
                selected_provider_model_name: "gpt-5".to_string(),
                provider_api_format: "openai:chat".to_string(),
                provider_type: "codex".to_string(),
                key_id: key_id.to_string(),
                key_internal_priority: internal_priority,
            },
            pool_config,
            key_context: AiPoolCatalogKeyContext::default(),
        }
    }

    trait TestCandidateExt {
        fn with_cost_limit(self, limit: u64) -> Self;
        fn with_presets(self, presets: Vec<AiPoolSchedulingPreset>) -> Self;
        fn with_plan(self, plan: &str) -> Self;
    }

    impl TestCandidateExt for AiPoolCandidateInput<String> {
        fn with_cost_limit(mut self, limit: u64) -> Self {
            if let Some(config) = self.pool_config.as_mut() {
                config.cost_limit_per_key_tokens = Some(limit);
            }
            self
        }

        fn with_presets(mut self, presets: Vec<AiPoolSchedulingPreset>) -> Self {
            if let Some(config) = self.pool_config.as_mut() {
                config.scheduling_presets = presets;
            }
            self
        }

        fn with_plan(mut self, plan: &str) -> Self {
            self.key_context.oauth_plan_type = Some(plan.to_string());
            self
        }
    }
}
