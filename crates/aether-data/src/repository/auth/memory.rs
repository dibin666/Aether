use std::collections::BTreeMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::types::{AuthApiKeyLookupKey, AuthApiKeyReadRepository, StoredAuthApiKeySnapshot};
use crate::DataLayerError;

#[derive(Debug, Default)]
struct MemoryAuthApiKeyIndex {
    by_api_key_id: BTreeMap<String, StoredAuthApiKeySnapshot>,
    by_key_hash: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct InMemoryAuthApiKeySnapshotRepository {
    index: RwLock<MemoryAuthApiKeyIndex>,
}

impl InMemoryAuthApiKeySnapshotRepository {
    pub fn seed<I>(items: I) -> Self
    where
        I: IntoIterator<Item = (Option<String>, StoredAuthApiKeySnapshot)>,
    {
        let mut by_api_key_id = BTreeMap::new();
        let mut by_key_hash = BTreeMap::new();
        for (key_hash, snapshot) in items {
            if let Some(key_hash) = key_hash {
                by_key_hash.insert(key_hash, snapshot.api_key_id.clone());
            }
            by_api_key_id.insert(snapshot.api_key_id.clone(), snapshot);
        }
        Self {
            index: RwLock::new(MemoryAuthApiKeyIndex {
                by_api_key_id,
                by_key_hash,
            }),
        }
    }
}

#[async_trait]
impl AuthApiKeyReadRepository for InMemoryAuthApiKeySnapshotRepository {
    async fn find_api_key_snapshot(
        &self,
        key: AuthApiKeyLookupKey<'_>,
    ) -> Result<Option<StoredAuthApiKeySnapshot>, DataLayerError> {
        let index = self
            .index
            .read()
            .expect("auth api key snapshot repository lock");
        Ok(match key {
            AuthApiKeyLookupKey::KeyHash(key_hash) => index
                .by_key_hash
                .get(key_hash)
                .and_then(|api_key_id| index.by_api_key_id.get(api_key_id))
                .cloned(),
            AuthApiKeyLookupKey::ApiKeyId(api_key_id) => {
                index.by_api_key_id.get(api_key_id).cloned()
            }
            AuthApiKeyLookupKey::UserApiKeyIds {
                user_id,
                api_key_id,
            } => index
                .by_api_key_id
                .get(api_key_id)
                .filter(|snapshot| snapshot.user_id == user_id)
                .cloned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryAuthApiKeySnapshotRepository;
    use crate::repository::auth::{
        AuthApiKeyLookupKey, AuthApiKeyReadRepository, StoredAuthApiKeySnapshot,
    };

    fn sample_snapshot(api_key_id: &str, user_id: &str) -> StoredAuthApiKeySnapshot {
        StoredAuthApiKeySnapshot::new(
            user_id.to_string(),
            "alice".to_string(),
            Some("alice@example.com".to_string()),
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            Some(serde_json::json!(["openai"])),
            Some(serde_json::json!(["openai:chat"])),
            Some(serde_json::json!(["gpt-4.1"])),
            api_key_id.to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(200),
            Some(serde_json::json!(["openai"])),
            Some(serde_json::json!(["openai:chat"])),
            Some(serde_json::json!(["gpt-4.1"])),
        )
        .expect("snapshot should build")
    }

    #[tokio::test]
    async fn reads_auth_snapshot_by_all_supported_keys() {
        let repository = InMemoryAuthApiKeySnapshotRepository::seed(vec![(
            Some("hash-1".to_string()),
            sample_snapshot("key-1", "user-1"),
        )]);

        assert!(repository
            .find_api_key_snapshot(AuthApiKeyLookupKey::KeyHash("hash-1"))
            .await
            .expect("find by hash should succeed")
            .is_some());
        assert!(repository
            .find_api_key_snapshot(AuthApiKeyLookupKey::ApiKeyId("key-1"))
            .await
            .expect("find by api key id should succeed")
            .is_some());
        assert!(repository
            .find_api_key_snapshot(AuthApiKeyLookupKey::UserApiKeyIds {
                user_id: "user-1",
                api_key_id: "key-1",
            })
            .await
            .expect("find by user/api key ids should succeed")
            .is_some());
    }
}
