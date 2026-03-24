use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredProviderCatalogProvider {
    pub id: String,
    pub name: String,
    pub website: Option<String>,
    pub provider_type: String,
}

impl StoredProviderCatalogProvider {
    pub fn new(
        id: String,
        name: String,
        website: Option<String>,
        provider_type: String,
    ) -> Result<Self, crate::DataLayerError> {
        if name.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "providers.name is empty".to_string(),
            ));
        }
        if provider_type.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "providers.provider_type is empty".to_string(),
            ));
        }

        Ok(Self {
            id,
            name,
            website,
            provider_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredProviderCatalogEndpoint {
    pub id: String,
    pub provider_id: String,
    pub api_format: String,
    pub api_family: Option<String>,
    pub endpoint_kind: Option<String>,
    pub is_active: bool,
}

impl StoredProviderCatalogEndpoint {
    pub fn new(
        id: String,
        provider_id: String,
        api_format: String,
        api_family: Option<String>,
        endpoint_kind: Option<String>,
        is_active: bool,
    ) -> Result<Self, crate::DataLayerError> {
        if api_format.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "provider_endpoints.api_format is empty".to_string(),
            ));
        }

        Ok(Self {
            id,
            provider_id,
            api_format,
            api_family,
            endpoint_kind,
            is_active,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredProviderCatalogKey {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub auth_type: String,
    pub capabilities: Option<serde_json::Value>,
    pub is_active: bool,
}

impl StoredProviderCatalogKey {
    pub fn new(
        id: String,
        provider_id: String,
        name: String,
        auth_type: String,
        capabilities: Option<serde_json::Value>,
        is_active: bool,
    ) -> Result<Self, crate::DataLayerError> {
        if name.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "provider_api_keys.name is empty".to_string(),
            ));
        }
        if auth_type.trim().is_empty() {
            return Err(crate::DataLayerError::UnexpectedValue(
                "provider_api_keys.auth_type is empty".to_string(),
            ));
        }

        Ok(Self {
            id,
            provider_id,
            name,
            auth_type,
            capabilities,
            is_active,
        })
    }
}

#[async_trait]
pub trait ProviderCatalogReadRepository: Send + Sync {
    async fn list_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, crate::DataLayerError>;

    async fn list_endpoints_by_ids(
        &self,
        endpoint_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogEndpoint>, crate::DataLayerError>;

    async fn list_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, crate::DataLayerError>;
}

#[cfg(test)]
mod tests {
    use super::{
        StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
    };

    #[test]
    fn rejects_empty_provider_name() {
        assert!(StoredProviderCatalogProvider::new(
            "provider-1".to_string(),
            "".to_string(),
            None,
            "custom".to_string(),
        )
        .is_err());
    }

    #[test]
    fn rejects_empty_endpoint_api_format() {
        assert!(StoredProviderCatalogEndpoint::new(
            "endpoint-1".to_string(),
            "provider-1".to_string(),
            "".to_string(),
            None,
            None,
            true,
        )
        .is_err());
    }

    #[test]
    fn rejects_empty_key_auth_type() {
        assert!(StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "provider-1".to_string(),
            "default".to_string(),
            "".to_string(),
            None,
            true,
        )
        .is_err());
    }
}
