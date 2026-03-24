use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredAuthApiKeySnapshot {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub user_role: String,
    pub user_auth_source: String,
    pub user_is_active: bool,
    pub user_is_deleted: bool,
    pub user_allowed_providers: Option<Vec<String>>,
    pub user_allowed_api_formats: Option<Vec<String>>,
    pub user_allowed_models: Option<Vec<String>>,
    pub api_key_id: String,
    pub api_key_name: Option<String>,
    pub api_key_is_active: bool,
    pub api_key_is_locked: bool,
    pub api_key_is_standalone: bool,
    pub api_key_rate_limit: Option<i32>,
    pub api_key_concurrent_limit: Option<i32>,
    pub api_key_expires_at_unix_secs: Option<u64>,
    pub api_key_allowed_providers: Option<Vec<String>>,
    pub api_key_allowed_api_formats: Option<Vec<String>>,
    pub api_key_allowed_models: Option<Vec<String>>,
}

impl StoredAuthApiKeySnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: String,
        username: String,
        email: Option<String>,
        user_role: String,
        user_auth_source: String,
        user_is_active: bool,
        user_is_deleted: bool,
        user_allowed_providers: Option<serde_json::Value>,
        user_allowed_api_formats: Option<serde_json::Value>,
        user_allowed_models: Option<serde_json::Value>,
        api_key_id: String,
        api_key_name: Option<String>,
        api_key_is_active: bool,
        api_key_is_locked: bool,
        api_key_is_standalone: bool,
        api_key_rate_limit: Option<i32>,
        api_key_concurrent_limit: Option<i32>,
        api_key_expires_at_unix_secs: Option<i64>,
        api_key_allowed_providers: Option<serde_json::Value>,
        api_key_allowed_api_formats: Option<serde_json::Value>,
        api_key_allowed_models: Option<serde_json::Value>,
    ) -> Result<Self, crate::DataLayerError> {
        Ok(Self {
            user_id,
            username,
            email,
            user_role,
            user_auth_source,
            user_is_active,
            user_is_deleted,
            user_allowed_providers: parse_string_list(
                user_allowed_providers,
                "users.allowed_providers",
            )?,
            user_allowed_api_formats: parse_string_list(
                user_allowed_api_formats,
                "users.allowed_api_formats",
            )?,
            user_allowed_models: parse_string_list(user_allowed_models, "users.allowed_models")?,
            api_key_id,
            api_key_name,
            api_key_is_active,
            api_key_is_locked,
            api_key_is_standalone,
            api_key_rate_limit,
            api_key_concurrent_limit,
            api_key_expires_at_unix_secs: api_key_expires_at_unix_secs
                .map(|value| {
                    u64::try_from(value).map_err(|_| {
                        crate::DataLayerError::UnexpectedValue(format!(
                            "invalid api_keys.expires_at_unix_secs: {value}"
                        ))
                    })
                })
                .transpose()?,
            api_key_allowed_providers: parse_string_list(
                api_key_allowed_providers,
                "api_keys.allowed_providers",
            )?,
            api_key_allowed_api_formats: parse_string_list(
                api_key_allowed_api_formats,
                "api_keys.allowed_api_formats",
            )?,
            api_key_allowed_models: parse_string_list(
                api_key_allowed_models,
                "api_keys.allowed_models",
            )?,
        })
    }

    pub fn is_currently_usable(&self, now_unix_secs: u64) -> bool {
        if !self.user_is_active || self.user_is_deleted {
            return false;
        }
        if !self.api_key_is_active {
            return false;
        }
        if self.api_key_is_locked && !self.api_key_is_standalone {
            return false;
        }
        if let Some(expires_at_unix_secs) = self.api_key_expires_at_unix_secs {
            if expires_at_unix_secs < now_unix_secs {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthApiKeyLookupKey<'a> {
    KeyHash(&'a str),
    ApiKeyId(&'a str),
    UserApiKeyIds {
        user_id: &'a str,
        api_key_id: &'a str,
    },
}

#[async_trait]
pub trait AuthApiKeyReadRepository: Send + Sync {
    async fn find_api_key_snapshot(
        &self,
        key: AuthApiKeyLookupKey<'_>,
    ) -> Result<Option<StoredAuthApiKeySnapshot>, crate::DataLayerError>;
}

pub trait AuthRepository: AuthApiKeyReadRepository + Send + Sync {}

impl<T> AuthRepository for T where T: AuthApiKeyReadRepository + Send + Sync {}

fn parse_string_list(
    value: Option<serde_json::Value>,
    field_name: &str,
) -> Result<Option<Vec<String>>, crate::DataLayerError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array = value.as_array().ok_or_else(|| {
        crate::DataLayerError::UnexpectedValue(format!("{field_name} is not a JSON array"))
    })?;
    let mut items = Vec::with_capacity(array.len());
    for item in array {
        let Some(item) = item.as_str() else {
            return Err(crate::DataLayerError::UnexpectedValue(format!(
                "{field_name} contains a non-string item"
            )));
        };
        items.push(item.to_string());
    }
    Ok(Some(items))
}

#[cfg(test)]
mod tests {
    use super::StoredAuthApiKeySnapshot;

    #[test]
    fn rejects_non_array_allowed_providers() {
        assert!(StoredAuthApiKeySnapshot::new(
            "user-1".to_string(),
            "alice".to_string(),
            None,
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            Some(serde_json::json!({"bad": true})),
            None,
            None,
            "key-1".to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            None,
            None,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn expired_non_standalone_key_is_not_usable() {
        let snapshot = StoredAuthApiKeySnapshot::new(
            "user-1".to_string(),
            "alice".to_string(),
            None,
            "user".to_string(),
            "local".to_string(),
            true,
            false,
            None,
            None,
            None,
            "key-1".to_string(),
            Some("default".to_string()),
            true,
            false,
            false,
            Some(60),
            Some(5),
            Some(100),
            None,
            None,
            None,
        )
        .expect("snapshot should build");

        assert!(!snapshot.is_currently_usable(101));
    }
}
