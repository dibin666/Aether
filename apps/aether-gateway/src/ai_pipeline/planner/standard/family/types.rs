use crate::ai_pipeline::control_facade::GatewayControlAuthContext;
use crate::ai_pipeline::planner::auth_snapshot_facade::GatewayAuthApiKeySnapshot;

pub(crate) use aether_ai_pipeline::planner::standard::family::{
    LocalStandardSourceFamily, LocalStandardSourceMode, LocalStandardSpec,
};

#[derive(Debug, Clone)]
pub(super) struct LocalStandardDecisionInput {
    pub(super) auth_context: GatewayControlAuthContext,
    pub(super) requested_model: String,
    pub(super) auth_snapshot: GatewayAuthApiKeySnapshot,
}

#[derive(Debug, Clone)]
pub(super) struct LocalStandardCandidateAttempt {
    pub(super) candidate: aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate,
    pub(super) candidate_index: u32,
    pub(super) candidate_id: String,
}
