use serde_json::Value;

use crate::GatewayError;
use aether_ai_pipeline::finalize::{self, PipelineFinalizeError};

pub(crate) use finalize::sse::{encode_done_sse, map_claude_stop_reason};

fn map_error(err: PipelineFinalizeError) -> GatewayError {
    err.into()
}

pub(crate) fn encode_json_sse(event: Option<&str>, value: &Value) -> Result<Vec<u8>, GatewayError> {
    finalize::sse::encode_json_sse(event, value).map_err(map_error)
}
