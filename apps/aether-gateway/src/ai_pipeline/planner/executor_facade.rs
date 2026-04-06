use aether_contracts::ExecutionPlan;
use serde_json::Value;

use crate::AppState;

pub(crate) async fn mark_unused_local_candidate_items<T, FPlan, FContext>(
    state: &AppState,
    remaining: Vec<T>,
    plan: FPlan,
    report_context: FContext,
) where
    FPlan: Fn(&T) -> &ExecutionPlan,
    FContext: Fn(&T) -> Option<&Value>,
{
    crate::executor::mark_unused_local_candidate_items(state, remaining, plan, report_context).await
}
