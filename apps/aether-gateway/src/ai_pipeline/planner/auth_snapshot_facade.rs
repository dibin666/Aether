pub(crate) use crate::data::auth::GatewayAuthApiKeySnapshot;
use crate::{AppState, GatewayError};

pub(crate) async fn read_auth_api_key_snapshot(
    state: &AppState,
    user_id: &str,
    api_key_id: &str,
    now_unix_secs: u64,
) -> Result<Option<GatewayAuthApiKeySnapshot>, GatewayError> {
    state
        .data
        .read_auth_api_key_snapshot(user_id, api_key_id, now_unix_secs)
        .await
        .map_err(|err| GatewayError::Internal(err.to_string()))
}
