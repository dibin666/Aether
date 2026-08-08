use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::{
    ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery, ProviderQuotaReadRepository,
    ProviderQuotaWriteRepository, StoredProviderQuotaSnapshot,
};
use crate::DataLayerError;
use aether_wallet::{ProviderBillingType, ProviderQuotaSnapshot};

#[derive(Debug, Default)]
pub struct InMemoryProviderQuotaRepository {
    by_provider_id: RwLock<BTreeMap<String, StoredProviderQuotaSnapshot>>,
    key_observations: RwLock<BTreeMap<(String, u64), ProviderKeyQuotaObservation>>,
}

impl InMemoryProviderQuotaRepository {
    pub fn seed<I>(items: I) -> Self
    where
        I: IntoIterator<Item = StoredProviderQuotaSnapshot>,
    {
        let mut by_provider_id = BTreeMap::new();
        for item in items {
            by_provider_id.insert(item.provider_id.clone(), item);
        }
        Self {
            by_provider_id: RwLock::new(by_provider_id),
            key_observations: RwLock::new(BTreeMap::new()),
        }
    }
}

#[async_trait]
impl ProviderQuotaReadRepository for InMemoryProviderQuotaRepository {
    async fn find_by_provider_id(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, DataLayerError> {
        Ok(self
            .by_provider_id
            .read()
            .expect("quota repository lock")
            .get(provider_id)
            .cloned())
    }

    async fn find_by_provider_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderQuotaSnapshot>, DataLayerError> {
        let quotas = self.by_provider_id.read().expect("quota repository lock");
        Ok(provider_ids
            .iter()
            .filter_map(|provider_id| quotas.get(provider_id).cloned())
            .collect())
    }

    async fn list_key_quota_observations(
        &self,
        query: &ProviderKeyQuotaObservationQuery,
    ) -> Result<Vec<ProviderKeyQuotaObservation>, DataLayerError> {
        let mut observations = self
            .key_observations
            .read()
            .expect("quota repository lock")
            .values()
            .filter(|item| item.provider_id == query.provider_id)
            .filter(|item| {
                query
                    .provider_api_key_id
                    .as_ref()
                    .is_none_or(|key_id| item.provider_api_key_id == *key_id)
            })
            .filter(|item| {
                query
                    .observed_from_unix_secs
                    .is_none_or(|from| item.observed_at_unix_secs >= from)
            })
            .filter(|item| {
                query
                    .observed_until_unix_secs
                    .is_none_or(|until| item.observed_at_unix_secs < until)
            })
            .cloned()
            .collect::<Vec<_>>();
        observations.sort_by(|left, right| {
            right
                .observed_at_unix_secs
                .cmp(&left.observed_at_unix_secs)
                .then_with(|| left.provider_api_key_id.cmp(&right.provider_api_key_id))
        });
        observations.truncate(query.limit.unwrap_or(usize::MAX));
        Ok(observations)
    }
}

#[async_trait]
impl ProviderQuotaWriteRepository for InMemoryProviderQuotaRepository {
    async fn reset_due(&self, now_unix_secs: u64) -> Result<usize, DataLayerError> {
        let mut count = 0usize;
        let mut quotas = self.by_provider_id.write().expect("quota repository lock");
        for quota in quotas.values_mut() {
            let snapshot = ProviderQuotaSnapshot {
                provider_id: quota.provider_id.clone(),
                billing_type: ProviderBillingType::parse(&quota.billing_type),
                monthly_quota_usd: quota.monthly_quota_usd,
                monthly_used_usd: quota.monthly_used_usd,
                quota_reset_day: quota.quota_reset_day,
                quota_last_reset_at_unix_secs: quota.quota_last_reset_at_unix_secs,
                quota_expires_at_unix_secs: quota.quota_expires_at_unix_secs,
                is_active: quota.is_active,
            };
            if snapshot.should_reset(now_unix_secs) {
                quota.monthly_used_usd = 0.0;
                quota.quota_last_reset_at_unix_secs = Some(now_unix_secs);
                count += 1;
            }
        }
        Ok(count)
    }

    async fn upsert_key_quota_observation(
        &self,
        observation: &ProviderKeyQuotaObservation,
    ) -> Result<bool, DataLayerError> {
        if observation.provider_id.trim().is_empty()
            || observation.provider_api_key_id.trim().is_empty()
        {
            return Err(DataLayerError::InvalidInput(
                "provider key quota observation identity is empty".to_string(),
            ));
        }
        let identity = (
            observation.provider_api_key_id.clone(),
            observation.bucket_start_unix_secs,
        );
        let mut observations = self
            .key_observations
            .write()
            .expect("quota repository lock");
        if observations.get(&identity).is_some_and(|current| {
            current.observed_at_unix_secs >= observation.observed_at_unix_secs
        }) {
            return Ok(false);
        }
        observations.insert(identity, observation.clone());
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryProviderQuotaRepository;
    use crate::repository::quota::{
        ProviderKeyQuotaObservation, ProviderKeyQuotaObservationQuery, ProviderQuotaReadRepository,
        ProviderQuotaWriteRepository, StoredProviderQuotaSnapshot,
    };

    fn sample_quota() -> StoredProviderQuotaSnapshot {
        StoredProviderQuotaSnapshot::new(
            "provider-1".to_string(),
            "monthly_quota".to_string(),
            Some(20.0),
            5.0,
            Some(7),
            Some(1_000),
            None,
            true,
        )
        .expect("quota should build")
    }

    #[tokio::test]
    async fn resets_due_monthly_quota() {
        let repository = InMemoryProviderQuotaRepository::seed(vec![sample_quota()]);
        let reset = repository
            .reset_due(1_000 + 7 * 24 * 60 * 60)
            .await
            .expect("reset should succeed");
        assert_eq!(reset, 1);
        let stored = repository
            .find_by_provider_id("provider-1")
            .await
            .expect("lookup should succeed")
            .expect("quota should exist");
        assert_eq!(stored.monthly_used_usd, 0.0);
    }

    #[tokio::test]
    async fn finds_quotas_by_provider_ids() {
        let repository = InMemoryProviderQuotaRepository::seed(vec![
            sample_quota(),
            StoredProviderQuotaSnapshot::new(
                "provider-2".to_string(),
                "payg".to_string(),
                None,
                1.5,
                None,
                None,
                None,
                true,
            )
            .expect("quota should build"),
        ]);

        let stored = repository
            .find_by_provider_ids(&[
                "provider-2".to_string(),
                "missing".to_string(),
                "provider-1".to_string(),
            ])
            .await
            .expect("lookup should succeed");

        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].provider_id, "provider-2");
        assert_eq!(stored[1].provider_id, "provider-1");
    }

    fn key_observation(observed_at: u64) -> ProviderKeyQuotaObservation {
        ProviderKeyQuotaObservation {
            provider_id: "provider-1".into(),
            provider_api_key_id: "key-1".into(),
            provider_api_key_name: "Key One".into(),
            provider_type: "codex".into(),
            bucket_start_unix_secs: 1_500,
            observed_at_unix_secs: observed_at,
            source: "test".into(),
            plan_type: None,
            status_code: None,
            status_label: None,
            freshness: None,
            credits_balance: None,
            credits_unlimited: None,
            reset_credits_count: 0,
            windows: Vec::new(),
        }
    }

    #[tokio::test]
    async fn key_observation_upsert_rejects_out_of_order_writes() {
        let repository = InMemoryProviderQuotaRepository::default();
        assert!(repository
            .upsert_key_quota_observation(&key_observation(1_700))
            .await
            .expect("first write should succeed"));
        assert!(!repository
            .upsert_key_quota_observation(&key_observation(1_650))
            .await
            .expect("old write should be ignored"));

        let stored = repository
            .list_key_quota_observations(&ProviderKeyQuotaObservationQuery {
                provider_id: "provider-1".into(),
                ..ProviderKeyQuotaObservationQuery::default()
            })
            .await
            .expect("history should load");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].observed_at_unix_secs, 1_700);
    }
}
