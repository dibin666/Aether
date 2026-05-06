use crate::handlers::admin::request::AdminAppState;
use crate::provider_key_auth::provider_key_is_oauth_managed;
use aether_admin::provider::pool as admin_provider_pool_pure;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use std::{cmp::Ordering, collections::BTreeMap};

const ADMIN_POOL_FREE_PLAN_DISPLAY_RANK: usize = 8;
const ADMIN_POOL_UNKNOWN_PLAN_DISPLAY_RANK: usize = 9;

pub(super) fn admin_pool_normalize_text(value: impl AsRef<str>) -> String {
    admin_provider_pool_pure::admin_pool_normalize_text(value)
}

fn admin_pool_parse_auth_config_json(
    state: &AdminAppState<'_>,
    key: &StoredProviderCatalogKey,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let ciphertext = key.encrypted_auth_config.as_deref()?.trim();
    if ciphertext.is_empty() {
        return None;
    }
    let plaintext = state.decrypt_catalog_secret_with_fallbacks(ciphertext)?;
    serde_json::from_str::<serde_json::Value>(&plaintext)
        .ok()?
        .as_object()
        .cloned()
}

fn admin_pool_derive_oauth_plan_type(
    state: &AdminAppState<'_>,
    key: &StoredProviderCatalogKey,
    provider_type: &str,
) -> Option<String> {
    let normalize = |value: &str| {
        let mut text = value.trim().to_string();
        if text.is_empty() {
            return None;
        }
        let provider_type = provider_type.trim().to_ascii_lowercase();
        if !provider_type.is_empty() && text.to_ascii_lowercase().starts_with(&provider_type) {
            text = text[provider_type.len()..]
                .trim_matches(|ch: char| [' ', ':', '-', '_'].contains(&ch))
                .to_string();
        }
        if text.is_empty() {
            None
        } else {
            Some(text.to_ascii_lowercase())
        }
    };

    if !provider_key_is_oauth_managed(key, provider_type) {
        return None;
    }

    if let Some(upstream_metadata) = key
        .upstream_metadata
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        let provider_bucket = upstream_metadata
            .get(&provider_type.trim().to_ascii_lowercase())
            .and_then(serde_json::Value::as_object);
        for source in provider_bucket
            .into_iter()
            .chain(std::iter::once(upstream_metadata))
        {
            for plan_key in [
                "plan_type",
                "tier",
                "subscription_title",
                "subscription_plan",
            ] {
                if let Some(value) = source.get(plan_key).and_then(serde_json::Value::as_str) {
                    if let Some(normalized) = normalize(value) {
                        return Some(normalized);
                    }
                }
            }
        }
    }

    if let Some(auth_config) = admin_pool_parse_auth_config_json(state, key) {
        for plan_key in ["plan_type", "tier", "plan", "subscription_plan"] {
            if let Some(value) = auth_config
                .get(plan_key)
                .and_then(serde_json::Value::as_str)
            {
                if let Some(normalized) = normalize(value) {
                    return Some(normalized);
                }
            }
        }
    }

    None
}

pub(super) fn admin_pool_matches_quick_selector(
    state: &AdminAppState<'_>,
    key: &StoredProviderCatalogKey,
    provider_type: &str,
    selector: &str,
) -> bool {
    let oauth_plan_type = admin_pool_derive_oauth_plan_type(state, key, provider_type);
    admin_provider_pool_pure::admin_pool_matches_quick_selector(
        key,
        selector,
        oauth_plan_type.as_deref(),
        admin_provider_pool_pure::admin_pool_now_unix_secs(),
        provider_type,
    )
}

pub(super) fn admin_pool_matches_search(
    state: &AdminAppState<'_>,
    key: &StoredProviderCatalogKey,
    provider_type: &str,
    search: Option<&str>,
) -> bool {
    let oauth_plan_type = admin_pool_derive_oauth_plan_type(state, key, provider_type);
    admin_provider_pool_pure::admin_pool_matches_search(key, search, oauth_plan_type.as_deref())
}

pub(super) fn admin_pool_key_is_known_banned(key: &StoredProviderCatalogKey) -> bool {
    admin_provider_pool_pure::admin_pool_key_is_known_banned(key)
}

fn admin_pool_display_plan_rank(oauth_plan_type: Option<&str>) -> usize {
    let Some(plan_type) = oauth_plan_type else {
        return ADMIN_POOL_UNKNOWN_PLAN_DISPLAY_RANK;
    };
    let normalized = plan_type.trim().to_ascii_lowercase();
    if normalized.contains("plus") {
        0
    } else if normalized.contains("team") {
        1
    } else if normalized.contains("pro") {
        2
    } else if normalized.contains("paid") {
        3
    } else if normalized.contains("enterprise") {
        4
    } else if normalized.contains("business") {
        5
    } else if normalized.contains("ultra") {
        6
    } else if normalized.contains("power") {
        7
    } else if normalized.contains("free") {
        ADMIN_POOL_FREE_PLAN_DISPLAY_RANK
    } else {
        ADMIN_POOL_UNKNOWN_PLAN_DISPLAY_RANK
    }
}

fn admin_pool_compare_keys_for_display(
    left: &StoredProviderCatalogKey,
    right: &StoredProviderCatalogKey,
    left_plan_type: Option<&str>,
    right_plan_type: Option<&str>,
) -> Ordering {
    let left_plan_rank = admin_pool_display_plan_rank(left_plan_type);
    let right_plan_rank = admin_pool_display_plan_rank(right_plan_type);
    let plan_order = left_plan_rank.cmp(&right_plan_rank);
    if plan_order != Ordering::Equal {
        return plan_order;
    }

    if left_plan_rank == ADMIN_POOL_FREE_PLAN_DISPLAY_RANK {
        let created_order = left
            .created_at_unix_ms
            .unwrap_or_default()
            .cmp(&right.created_at_unix_ms.unwrap_or_default());
        if created_order != Ordering::Equal {
            return created_order;
        }
    }

    left.internal_priority
        .cmp(&right.internal_priority)
        .then(left.name.cmp(&right.name))
        .then(
            left.created_at_unix_ms
                .unwrap_or_default()
                .cmp(&right.created_at_unix_ms.unwrap_or_default()),
        )
        .then(left.id.cmp(&right.id))
}

pub(super) fn admin_pool_sort_keys(
    state: &AdminAppState<'_>,
    provider_type: &str,
    keys: &mut [StoredProviderCatalogKey],
) {
    let plan_by_key_id = keys
        .iter()
        .map(|key| {
            (
                key.id.clone(),
                admin_pool_derive_oauth_plan_type(state, key, provider_type),
            )
        })
        .collect::<BTreeMap<_, _>>();

    keys.sort_by(|left, right| {
        let left_plan = plan_by_key_id
            .get(&left.id)
            .and_then(|value| value.as_deref());
        let right_plan = plan_by_key_id
            .get(&right.id)
            .and_then(|value| value.as_deref());
        admin_pool_compare_keys_for_display(left, right, left_plan, right_plan)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sort_key(id: &str, name: &str, priority: i32, created_at: u64) -> StoredProviderCatalogKey {
        let mut key = StoredProviderCatalogKey::new(
            id.to_string(),
            "provider-pool".to_string(),
            name.to_string(),
            "oauth".to_string(),
            None,
            true,
        )
        .expect("key should be valid");
        key.internal_priority = priority;
        key.created_at_unix_ms = Some(created_at);
        key
    }

    #[test]
    fn display_sort_keeps_paid_plans_before_free_keys() {
        let mut keys = vec![
            (sort_key("free-new", "free-new", 1, 300), Some("free")),
            (sort_key("team", "team", 50, 100), Some("team")),
            (sort_key("free-old", "free-old", 99, 100), Some("free")),
            (sort_key("plus", "plus", 99, 200), Some("plus")),
        ];

        keys.sort_by(|(left, left_plan), (right, right_plan)| {
            admin_pool_compare_keys_for_display(left, right, *left_plan, *right_plan)
        });

        let ordered_ids = keys
            .iter()
            .map(|(key, _)| key.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ordered_ids, vec!["plus", "team", "free-old", "free-new"]);
    }
}
