use aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate;

use crate::ai_pipeline::planner::auth_snapshot_facade::GatewayAuthApiKeySnapshot;
use crate::{AppState, GatewayError};

pub(crate) async fn list_selectable_candidates(
    state: &AppState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    crate::scheduler::candidate::list_selectable_candidates(
        state.data.as_ref(),
        state,
        api_format,
        global_model_name,
        require_streaming,
        auth_snapshot,
        now_unix_secs,
    )
    .await
}

pub(crate) async fn list_selectable_candidates_for_required_capability_without_requested_model(
    state: &AppState,
    candidate_api_format: &str,
    required_capability: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    crate::scheduler::candidate::list_selectable_candidates_for_required_capability_without_requested_model(
        state.data.as_ref(),
        state,
        candidate_api_format,
        required_capability,
        require_streaming,
        auth_snapshot,
        now_unix_secs,
    )
    .await
}
