use crate::ai_pipeline::control_facade::GatewayControlAuthContext;
use crate::ai_pipeline::planner::auth_snapshot_facade::GatewayAuthApiKeySnapshot;

pub(crate) use aether_ai_pipeline::planner::passthrough::provider::{
    LocalSameFormatProviderFamily, LocalSameFormatProviderSpec,
};

#[derive(Debug, Clone)]
pub(crate) struct LocalSameFormatProviderDecisionInput {
    pub(crate) auth_context: GatewayControlAuthContext,
    pub(crate) requested_model: String,
    pub(crate) auth_snapshot: GatewayAuthApiKeySnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalSameFormatProviderCandidateAttempt {
    pub(crate) candidate: aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate,
    pub(crate) candidate_index: u32,
    pub(crate) candidate_id: String,
}
