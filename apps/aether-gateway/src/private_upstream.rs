//! Runtime registry of administrator-granted private upstream allowances.
//!
//! [`crate::provider_transport::private_network`] decides *what* a saved
//! provider endpoint is allowed to reach. This module decides *when* the
//! execution path may see that decision, and the answer is deliberately narrow:
//! entries are written only while loading a provider transport snapshot out of
//! the database, and they are keyed by the endpoint and key that snapshot
//! belongs to.
//!
//! Nothing on the request path can create an entry. An execution plan names an
//! endpoint and key, but naming them only selects which allowance to compare
//! against — a plan that points at an internal address the administrator never
//! saved still has nothing to match and stays refused.

use std::sync::LazyLock;

use dashmap::DashMap;
use serde_json::Value;

use crate::provider_transport::private_network::{
    resolve_endpoint_private_upstream_origin, PrivateUpstreamOrigin,
};

#[derive(Clone, PartialEq, Eq, Hash)]
struct PrivateUpstreamAllowanceKey {
    endpoint_id: String,
    key_id: String,
}

/// Granted allowances, keyed per endpoint and key.
///
/// The key pair matters: a provider key can carry a local auth config that
/// rewrites the endpoint base URL for that key alone, so an allowance derived
/// for one key must not leak to a sibling key on the same endpoint.
static PRIVATE_UPSTREAM_ALLOWANCES: LazyLock<
    DashMap<PrivateUpstreamAllowanceKey, PrivateUpstreamOrigin>,
> = LazyLock::new(DashMap::new);

/// Refresh the allowance for one endpoint and key from freshly loaded rows.
///
/// Called for every provider transport snapshot load, so a revoked or edited
/// allowance disappears as soon as the snapshot cache is invalidated and
/// reloaded. `base_url` and `config` must come from the same snapshot that will
/// build the upstream URL, including any per-key local auth absorption.
pub(crate) fn sync_endpoint_private_upstream_allowance(
    endpoint_id: &str,
    key_id: &str,
    base_url: &str,
    config: Option<&Value>,
) {
    let granted = resolve_endpoint_private_upstream_origin(base_url, config)
        .ok()
        .flatten();
    if granted.is_none() && PRIVATE_UPSTREAM_ALLOWANCES.is_empty() {
        return;
    }

    let allowance_key = PrivateUpstreamAllowanceKey {
        endpoint_id: endpoint_id.to_string(),
        key_id: key_id.to_string(),
    };
    let previous = match granted {
        Some(origin) => PRIVATE_UPSTREAM_ALLOWANCES.insert(allowance_key, origin),
        None => PRIVATE_UPSTREAM_ALLOWANCES
            .remove(&allowance_key)
            .map(|(_, origin)| origin),
    };
    if previous == granted {
        return;
    }

    match granted {
        Some(origin) => tracing::warn!(
            event_name = "provider_private_upstream_allowance_granted",
            endpoint_id = %endpoint_id,
            key_id = %key_id,
            allowed_origin = %origin.origin(),
            "provider endpoint is configured to reach a private upstream address"
        ),
        None => tracing::info!(
            event_name = "provider_private_upstream_allowance_revoked",
            endpoint_id = %endpoint_id,
            key_id = %key_id,
            "provider endpoint no longer reaches a private upstream address"
        ),
    }
}

/// The allowance currently granted to one endpoint and key, if any.
pub(crate) fn private_upstream_allowance(
    endpoint_id: &str,
    key_id: &str,
) -> Option<PrivateUpstreamOrigin> {
    if PRIVATE_UPSTREAM_ALLOWANCES.is_empty() {
        return None;
    }
    PRIVATE_UPSTREAM_ALLOWANCES
        .get(&PrivateUpstreamAllowanceKey {
            endpoint_id: endpoint_id.to_string(),
            key_id: key_id.to_string(),
        })
        .map(|entry| *entry.value())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // The registry is process wide, so the cases below use distinct endpoint
    // ids instead of sharing one and relying on test ordering.
    #[test]
    fn snapshot_loads_grant_revoke_and_scope_allowances() {
        let enabled = json!({"private_network_access": {"enabled": true}});
        let disabled = json!({"private_network_access": {"enabled": false}});

        sync_endpoint_private_upstream_allowance(
            "endpoint-grant",
            "key-a",
            "http://10.0.0.106:8317/v1",
            Some(&enabled),
        );
        assert_eq!(
            private_upstream_allowance("endpoint-grant", "key-a")
                .expect("an enabled endpoint should be granted")
                .origin(),
            "http://10.0.0.106:8317"
        );
        // A sibling key on the same endpoint has not been loaded, so it has no
        // allowance of its own.
        assert!(private_upstream_allowance("endpoint-grant", "key-b").is_none());

        sync_endpoint_private_upstream_allowance(
            "endpoint-grant",
            "key-a",
            "http://10.0.0.106:8317/v1",
            Some(&disabled),
        );
        assert!(private_upstream_allowance("endpoint-grant", "key-a").is_none());
    }

    #[test]
    fn rejected_configurations_never_reach_the_registry() {
        for (endpoint_id, base_url, config) in [
            (
                "endpoint-public",
                "https://api.example.test/v1",
                json!({"private_network_access": {"enabled": true}}),
            ),
            (
                "endpoint-hostname",
                "http://internal.corp.test:8317/v1",
                json!({"private_network_access": {"enabled": true}}),
            ),
            (
                "endpoint-malformed",
                "http://10.0.0.106:8317/v1",
                json!({"private_network_access": {"enabled": "true"}}),
            ),
            (
                "endpoint-mismatched-pin",
                "http://10.0.0.106:8317/v1",
                json!({"private_network_access": {"enabled": true, "target": "10.0.0.107:8317"}}),
            ),
        ] {
            sync_endpoint_private_upstream_allowance(endpoint_id, "key-a", base_url, Some(&config));
            assert!(
                private_upstream_allowance(endpoint_id, "key-a").is_none(),
                "{endpoint_id} must not be granted an allowance"
            );
        }
    }
}
