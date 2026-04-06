use crate::AppState;

use super::super::super::{AUTH_API_KEY_LAST_USED_MAX_ENTRIES, AUTH_API_KEY_LAST_USED_TTL};

impl AppState {
    pub(crate) async fn touch_auth_api_key_last_used_best_effort(&self, api_key_id: &str) {
        let api_key_id = api_key_id.trim();
        if api_key_id.is_empty() || !self.data.has_auth_api_key_writer() {
            return;
        }
        if !self.auth_api_key_last_used_cache.should_touch(
            api_key_id,
            AUTH_API_KEY_LAST_USED_TTL,
            AUTH_API_KEY_LAST_USED_MAX_ENTRIES,
        ) {
            return;
        }
        if let Err(err) = self.data.touch_auth_api_key_last_used(api_key_id).await {
            tracing::warn!(
                api_key_id = %api_key_id,
                error = ?err,
                "gateway auth api key last_used_at touch failed"
            );
        }
    }
}
