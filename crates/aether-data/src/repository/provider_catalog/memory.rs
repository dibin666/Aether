use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{
    ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
    StoredProviderCatalogProvider,
};
use crate::DataLayerError;

#[derive(Debug, Default)]
struct MemoryProviderCatalogIndex {
    providers: BTreeMap<String, StoredProviderCatalogProvider>,
    endpoints: BTreeMap<String, StoredProviderCatalogEndpoint>,
    keys: BTreeMap<String, StoredProviderCatalogKey>,
}

#[derive(Debug, Default)]
pub struct InMemoryProviderCatalogReadRepository {
    index: RwLock<MemoryProviderCatalogIndex>,
}

impl InMemoryProviderCatalogReadRepository {
    pub fn seed(
        providers: Vec<StoredProviderCatalogProvider>,
        endpoints: Vec<StoredProviderCatalogEndpoint>,
        keys: Vec<StoredProviderCatalogKey>,
    ) -> Self {
        Self {
            index: RwLock::new(MemoryProviderCatalogIndex {
                providers: providers
                    .into_iter()
                    .map(|provider| (provider.id.clone(), provider))
                    .collect(),
                endpoints: endpoints
                    .into_iter()
                    .map(|endpoint| (endpoint.id.clone(), endpoint))
                    .collect(),
                keys: keys.into_iter().map(|key| (key.id.clone(), key)).collect(),
            }),
        }
    }
}

#[async_trait]
impl ProviderCatalogReadRepository for InMemoryProviderCatalogReadRepository {
    async fn list_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, DataLayerError> {
        let index = self.index.read().expect("provider catalog repository lock");
        Ok(provider_ids
            .iter()
            .filter_map(|id| index.providers.get(id).cloned())
            .collect())
    }

    async fn list_endpoints_by_ids(
        &self,
        endpoint_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogEndpoint>, DataLayerError> {
        let index = self.index.read().expect("provider catalog repository lock");
        Ok(endpoint_ids
            .iter()
            .filter_map(|id| index.endpoints.get(id).cloned())
            .collect())
    }

    async fn list_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, DataLayerError> {
        let index = self.index.read().expect("provider catalog repository lock");
        Ok(key_ids
            .iter()
            .filter_map(|id| index.keys.get(id).cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryProviderCatalogReadRepository;
    use crate::repository::provider_catalog::{
        ProviderCatalogReadRepository, StoredProviderCatalogEndpoint, StoredProviderCatalogKey,
        StoredProviderCatalogProvider,
    };

    fn sample_provider(id: &str) -> StoredProviderCatalogProvider {
        StoredProviderCatalogProvider::new(
            id.to_string(),
            format!("provider-{id}"),
            Some("https://example.com".to_string()),
            "custom".to_string(),
        )
        .expect("provider should build")
    }

    fn sample_endpoint(id: &str, provider_id: &str) -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            id.to_string(),
            provider_id.to_string(),
            "openai:chat".to_string(),
            Some("openai".to_string()),
            Some("chat".to_string()),
            true,
        )
        .expect("endpoint should build")
    }

    fn sample_key(id: &str, provider_id: &str) -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new(
            id.to_string(),
            provider_id.to_string(),
            "default".to_string(),
            "api_key".to_string(),
            Some(serde_json::json!({"cache_1h": true})),
            true,
        )
        .expect("key should build")
    }

    #[tokio::test]
    async fn reads_provider_catalog_items_by_id() {
        let repository = InMemoryProviderCatalogReadRepository::seed(
            vec![sample_provider("provider-1")],
            vec![sample_endpoint("endpoint-1", "provider-1")],
            vec![sample_key("key-1", "provider-1")],
        );

        assert_eq!(
            repository
                .list_providers_by_ids(&["provider-1".to_string()])
                .await
                .expect("providers should read")
                .len(),
            1
        );
        assert_eq!(
            repository
                .list_endpoints_by_ids(&["endpoint-1".to_string()])
                .await
                .expect("endpoints should read")
                .len(),
            1
        );
        assert_eq!(
            repository
                .list_keys_by_ids(&["key-1".to_string()])
                .await
                .expect("keys should read")
                .len(),
            1
        );
    }
}
