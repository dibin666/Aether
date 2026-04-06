use aether_data_contracts::repository::video_tasks::VideoTaskLookupKey;
use aether_usage_runtime::build_locally_actionable_report_context_from_video_task;
use serde_json::Value;

use crate::request_candidate_runtime::resolve_locally_actionable_request_candidate_report_context;
use crate::video_tasks::{resolve_video_task_report_lookup, VideoTaskReportLookup};
use crate::AppState;

pub(crate) use aether_usage_runtime::report_context_is_locally_actionable;

pub(crate) async fn resolve_locally_actionable_report_context(
    state: &AppState,
    report_context: Option<&Value>,
) -> Option<Value> {
    let context = report_context?.clone();
    if report_context_is_locally_actionable(Some(&context)) {
        return Some(context);
    }

    if let Some(resolved) =
        resolve_locally_actionable_request_candidate_report_context(state, &context).await
    {
        return Some(resolved);
    }

    let context = resolve_locally_actionable_report_context_from_video_task(state, &context)
        .await
        .unwrap_or(context);

    if let Some(resolved) =
        resolve_locally_actionable_request_candidate_report_context(state, &context).await
    {
        return Some(resolved);
    }

    report_context_is_locally_actionable(Some(&context)).then_some(context)
}

async fn resolve_locally_actionable_report_context_from_video_task(
    state: &AppState,
    context: &Value,
) -> Option<Value> {
    let task = match resolve_video_task_report_lookup(context)? {
        VideoTaskReportLookup::Lookup(lookup) => {
            state.data.find_video_task(lookup).await.ok()??
        }
        VideoTaskReportLookup::TaskIdOrExternal { task_id, user_id } => {
            if let Some(task) = state
                .data
                .find_video_task(VideoTaskLookupKey::Id(task_id))
                .await
                .ok()?
            {
                task
            } else {
                let user_id = user_id?;
                state
                    .data
                    .find_video_task(VideoTaskLookupKey::UserExternal {
                        user_id,
                        external_task_id: task_id,
                    })
                    .await
                    .ok()??
            }
        }
    };

    build_locally_actionable_report_context_from_video_task(context, &task)
}
