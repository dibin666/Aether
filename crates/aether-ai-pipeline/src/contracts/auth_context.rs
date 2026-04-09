use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionRuntimeAuthContext {
    pub user_id: String,
    pub api_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_name: Option<String>,
    pub balance_remaining: Option<f64>,
    pub access_allowed: bool,
}
